#![allow(unused)]

mod make;

use clap::{Parser, Subcommand};
use sbwt::SeqStream;

use simple_sds_sbwt::ops::{BitVec, PredSucc, Rank, Select};
use simple_sds_sbwt::raw_vector::{RawVector, AccessRaw};
use simple_sds_sbwt::bit_vector::BitVector;
use simple_sds_sbwt::serialize::Serialize;

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write, Read};
use std::path::PathBuf;

use make::*;

#[derive(Parser)]
#[command(about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(about, long_about)]
enum Commands {
    /// Concatenate the input sequences stored in files, given in a list file.
    ///
    /// Output file extension should be ".concat".
    #[command(name = "concat")]
    Concatenate {
        /// A file with filenames on separate lines.
        #[arg(short = 'l')]
        file_list_path: PathBuf,

        /// The directory in which the files are located.
        ///
        /// By default searches for the files in the current working directory.
        #[arg(short = 'd')]
        directory_with_sequence_files: Option<PathBuf>,

        /// Output path for the concatenated input sequences. Defaults to "./result.concat".
        #[arg(short = 'o')]
        output_path: Option<PathBuf>,
    },
    /// Truncate numbers from LCP array.
    ///
    /// Input file extension should be ".lcp". Output file extensions should be ".lcpt".
    ///
    /// The integers in the output file are little endian.
    #[command(name = "trunc")]
    TruncateLcp {
        /// The path to the input LCP.
        #[arg(short = 'i')]
        input_path: PathBuf,

        /// The path to the output LCP. Defaults to "./result.lcpt".
        #[arg(short = 'o')]
        output_path: Option<PathBuf>,

        /// The upper bound to truncate the values in the LCP.
        #[arg(short = 'k')]
        max_k: u32,

        /// Treat the integers in the input file as big endian.
        #[arg(default_value_t = false, long = "big_endian")]
        big_endian: bool,
    },
    /// Make bitvectors from BWT.
    ///
    /// Input file extension should be ".bwt". Output file extensions should be ".bwtb".
    #[command(name = "bwt")]
    BwtBitVectors {
        /// The path to the input BWT. The expected characters are {'$', 'A', 'C', 'G', 'T'}.
        #[arg(short = 'i')]
        input_path: PathBuf,

        /// The path to the output LCP. Defaults to "./result.bwtb".
        #[arg(short = 'o')]
        output_path: Option<PathBuf>,
    },
    /// Construct SBWT out of a LCP array and a BWT of the concatenated input sequences.
    #[command(name = "build")]
    Build {
        /// The path to the input BWT bitvectors.
        #[arg(short = 'b')]
        bwtb_path: PathBuf,

        /// The path to the input (truncated) LCP.
        #[arg(short = 'l')]
        lcpt_path: PathBuf,

        /// The value of k.
        #[arg(short = 'k')]
        k: u32,

        /// The path to the output. Defaults to "./result.bsbwt".
        #[arg(short = 'o')]
        output_path: Option<PathBuf>,
    }
}

fn main() {
    let Cli { command } = Cli::parse();
    use Commands::*;
    match command {
        Concatenate {
            file_list_path,
            directory_with_sequence_files,
            output_path,
        } => {
            concatenate(file_list_path, directory_with_sequence_files, output_path).unwrap();
        }
        TruncateLcp {
            input_path,
            output_path,
            max_k,
            big_endian,
        } => {
            if !big_endian {
                truncate_lcp::<false>(input_path, output_path, max_k).unwrap();
            } else {
                truncate_lcp::<true>(input_path, output_path, max_k).unwrap();
            }
        }
        BwtBitVectors {
            input_path,
            output_path
        } => {
            bwt_bit_vectors(input_path, output_path).unwrap();
        },
        Build {
            bwtb_path,
            lcpt_path,
            output_path,
            k
        } => {
            build(bwtb_path, lcpt_path, output_path, k).unwrap()
        },
    };
}

fn concatenate(
    file_list_path: PathBuf,
    directory_with_sequence_files: Option<PathBuf>,
    output_path: Option<PathBuf>,
) -> std::io::Result<()> {
    let input_sequences = read_lines(&file_list_path)?;
    if let Some(dir) = directory_with_sequence_files {
        std::env::set_current_dir(dir)?;
    }
    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.concat"),
    };
    let output_file = File::create(output_path)?;
    let mut output_writer = BufWriter::new(output_file);
    let mut sequence_reader = SeqReader::new(&input_sequences);
    write_concatenation(sequence_reader, &mut output_writer)?;
    Ok(())
}

pub fn write_concatenation<SS: SeqStream + Send, W: std::io::Write>(mut stream: SS, output: &mut W) -> std::io::Result<()> {
    write!(output, "$")?;
    while let Some(sequence) = stream.stream_next() {
        write!(output, "$")?;
        output.write_all(sequence)?;
    }
    write!(output, "$")?;
    Ok(())
}

fn truncate_lcp<const BIG_ENDIAN: bool>(
    input_path: PathBuf,
    output_path: Option<PathBuf>,
    max_k: u32,
) -> std::io::Result<()> {
    let input_file = File::open(input_path)?;
    let mut input_reader = BufReader::new(input_file);

    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.lcpt"),
    };
    let output_file = File::create(output_path)?;
    let mut output_writer = BufWriter::new(output_file);

    let mut bytes = [0_u8; size_of::<u64>()];
    let k_bit_width = u32::BITS - max_k.leading_zeros();
    let output_byte_count = (k_bit_width.div_ceil(u8::BITS) as usize).next_power_of_two();

    while input_reader.read_exact(&mut bytes).is_ok() {
        let number = if BIG_ENDIAN {
            u64::from_be_bytes(bytes)
        } else {
            u64::from_le_bytes(bytes)
        };
        let truncated_number = number.min(max_k as u64);
        let result_bytes = &truncated_number.to_le_bytes()[..output_byte_count];
        output_writer.write_all(result_bytes)?;
    }
    Ok(())
}

fn bwt_bit_vectors(
    input_path: PathBuf,
    output_path: Option<PathBuf>,
) -> std::io::Result<()> {
    assert_eq!(usize::BITS, u64::BITS, "Use a 64-bit machine pretty please.");
    let input_file = File::open(input_path)?;
    let metadata = input_file.metadata()?;
    let mut input_reader = BufReader::new(input_file);
    let len = metadata.len() as usize;

    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.bwtb"),
    };
    let output_file = File::create(output_path)?;
    let mut output_writer = BufWriter::new(output_file);

    let mut raw_vectors = [
        RawVector::with_len(len, false), // $
        RawVector::with_len(len, false), // A
        RawVector::with_len(len, false), // C
        RawVector::with_len(len, false), // G
        RawVector::with_len(len, false), // T
    ];

    let mut byte = [0_u8];
    let mut index: usize = 0;
    while input_reader.read_exact(&mut byte).is_ok() {
        let char_index = char_index(byte[0]);
        raw_vectors[char_index].set_bit(index, true);
        index += 1;
    }

    let mut bit_vectors = raw_vectors.into_iter().map(BitVector::from).collect::<Vec<_>>();
    bit_vectors.iter_mut().for_each(|vector| {
        vector.enable_rank();
        vector.enable_select();
    });

    for bit_vector in bit_vectors {
        bit_vector.serialize(&mut output_writer)?;
    }
    Ok(())
}

fn build(bwtb_path: PathBuf, lcpt_path: PathBuf, output_path: Option<PathBuf>, k: u32) -> std::io::Result<()> {
    let bwtb_file = File::open(bwtb_path)?;
    let mut lcpt_file = File::open(lcpt_path)?;

    let mut bwtb_reader = BufReader::new(bwtb_file);
    let bwt = Bwt::load(&mut bwtb_reader)?;

    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.bsbwt"),
    };
    let mut output_file = File::options().write(true).open(output_path)?;

    let lcpt_metadata = lcpt_file.metadata()?;
    let mut lcp_data: Vec<u8> = Vec::with_capacity(lcpt_metadata.len() as usize);
    lcpt_file.read_to_end(&mut lcp_data)?;

    let k_bit_width = u32::BITS - k.leading_zeros();
    let byte_count = (k_bit_width.div_ceil(u8::BITS) as usize).next_power_of_two();
    let mut lcp = if byte_count < 2 {
        Lcp::new::<u8>(lcp_data)
    } else if byte_count < 4 {
        Lcp::new::<u16>(lcp_data)
    } else {
        Lcp::new::<u32>(lcp_data)
    };

    let k = k as usize;

    let separator_count = bwt.data[char_index(b'$')].count_ones();
    let ranges = calculate_ranges(&mut lcp, k, separator_count);

    let (shorter_than_k, equal_to_k) = calculate_length_marker_bitvectors(&bwt, k);
    let (keep_suffix, keep_letter) = calculate_dummy_marks(
        &bwt, &mut lcp, k, &ranges, &shorter_than_k, &equal_to_k
    );

    drop(equal_to_k);
    let sets = build_sets(&bwt, &mut lcp, &ranges, &shorter_than_k, &keep_suffix, &keep_letter);
    output_file.write_all(&sets);

    Ok(())
}

fn calculate_ranges(lcp: &mut Lcp, k: usize, separator_count: usize) -> BitVector {
    lcp.reset();
    let len = lcp.len();
    let mut ranges = RawVector::with_len(len, false);
    ranges.set_bit(0, true);

    // Skip the region of the F array which contains the '$' symbols.
    Lcp::skip(lcp, separator_count);
    let mut index = separator_count;

    // The prefix of a suffix up to the first '$' will be referred to as the true prefix and
    // its length as the true length.
    //
    // An N-region is a contiguous region of suffixes which have the same true prefix
    // truncated from the right to a length of N (or if the true length of the suffix is less
    // than N, they are padded with imaginary '$').
    //
    // A (k-1)-region which contains suffixes with true lengths less than k-1 will be referred
    // to as a small region. A (k-1)-region which contains suffixes with true length equal to k
    // will be referred to as a big region.
    //
    // A big region can be further divided into k-regions. The LCP array will "lie" about the
    // true length of the suffix. For example, the second from the following two will have an
    // LCP value of at least 3:
    //
    // A$A...
    // A$A...
    //

    let mut target_value = 0;
    let max_step = k - 2;
    #[allow(clippy::explicit_counter_loop)]
    for value in lcp {
        if value <= target_value {
            ranges.set_bit(index, true);
            target_value = (value + 1).min(max_step);
        }
        index += 1;
    }
    let mut ranges = BitVector::from(ranges);
    ranges.enable_pred_succ();
    ranges
}

fn calculate_length_marker_bitvectors(bwt: &Bwt, k: usize) -> (BitVector, RawVector) {
    let len = bwt.len();
    let mut shorter_than_k = RawVector::with_len(len, false);
    let mut equal_to_k     = RawVector::with_len(len, false);
    let mut order = 0;
    let mut current_length = 0;
    for _ in 0..len {
        let (next_order, character) = bwt.lf_step(order);
        order = next_order;
        if character == b'$' {
            current_length = 0;
        } else {
            current_length += 1;
        }
        if current_length < k {
            shorter_than_k.set_bit(order, true);
        } else if current_length == k {
            equal_to_k.set_bit(order, true);
        }
    }
    let mut shorter_than_k = BitVector::from(shorter_than_k);
    shorter_than_k.enable_rank();
    (shorter_than_k, equal_to_k)
}

fn calculate_dummy_marks(
    bwt: &Bwt,
    lcp: &mut Lcp,
    k: usize,
    ranges: &BitVector,
    shorter_than_k: &BitVector,
    equal_to_k: &RawVector,
) -> (RawVector, RawVector) {
    lcp.reset();
    let len = bwt.len();
    let mut keep_suffix = RawVector::with_len(len, false);
    let mut keep_letter = RawVector::with_len(len, false);

    // Skip the region of the F array which contains the '$' symbols.
    let mut index = bwt.data[char_index(b'$')].count_ones();
    Lcp::skip(lcp, index);

    let mut has_full_kmer_as_predecessor = false;
    // We want to iterate through the k-ranges as well this time.
    let mut target_value = 0;
    let max_step = k - 1;
    #[allow(clippy::explicit_counter_loop)]
    for value in lcp {
        if value <= target_value {
            target_value = (value + 1).min(max_step);
            has_full_kmer_as_predecessor = false;
        }

        if equal_to_k.bit(index) {
            let predecessor = bwt.inverse_lf_step(index);

            // If we haven't found a full k-mer as a predecessor for this k-region, search for it.
            if !has_full_kmer_as_predecessor {
                has_full_kmer_as_predecessor |= has_full_kmer_predecessor(
                    predecessor, bwt, ranges, shorter_than_k
                );
                println!("i: {}; {}", index, has_full_kmer_as_predecessor);
            }

            if has_full_kmer_as_predecessor {
                let predecessor = bwt.inverse_lf_step(index);
                keep_letter.set_bit(predecessor, true);
            } else {
                keep_predecessors(predecessor, bwt, k, &mut keep_suffix);
            }
        } else if !shorter_than_k.get(index) {
            // If the true length of the prefix of this suffix is not equal to k and it is not
            // shorter than k, this means that it is longer than k. If this is the case, this means
            // that this k-region has a predecessor.
            has_full_kmer_as_predecessor = true;
        }

        index += 1;
    }

    (keep_suffix, keep_letter)
}

fn has_full_kmer_predecessor(
    predecessor: usize,
    bwt: &Bwt,
    ranges: &BitVector, 
    shorter_than_k: &BitVector
) -> bool {
    let range_start = predecessor;
    let one_index = ranges.rank(range_start + 1);
    let range_end = if one_index == ranges.count_ones() {
        bwt.len()
    } else {
        // There is at least one 1 after the current position.
        ranges.select(one_index).unwrap()
    };
    let range_length = range_end - range_start;
    let number_of_prefixes_with_true_length_smaller_than_k =
        shorter_than_k.rank(range_end) - shorter_than_k.rank(range_start);
    number_of_prefixes_with_true_length_smaller_than_k < range_length
}

fn keep_predecessors(mut predecessor: usize, bwt: &Bwt, mut k: usize, keep_suffix: &mut RawVector) {
    while k > 0 {
        keep_suffix.set_bit(predecessor, true);
        predecessor = bwt.inverse_lf_step(predecessor);
        k -= 1;
    }
}

fn build_sets(
    bwt: &Bwt,
    lcp: &mut Lcp,
    ranges: &BitVector,
    shorter_than_k: &BitVector,
    // equal_to_k: &RawVector,
    keep_suffix: &RawVector,
    keep_letter: &RawVector,
) -> Vec<u8> {
    lcp.reset();

    let mut sets = Vec::<u8>::with_capacity(bwt.len());

    let mut separator_count = bwt.data[char_index(b'$')].count_ones();
    let mut current_set: u8 = 0;
    {
        // Separator symbol region.
        for index in 0..separator_count {
            if keep_suffix.bit(index) {
                current_set |= (1 << bwt.get_char_index(index) as u8);
            }
        }
        sets.push(current_set);
    }

    lcp.skip(separator_count);
    let mut emit_set = false;
    current_set = 0;

    // for (index, value) in lcp.enumerate() {
    //     if value <= target_value {
    //         target_value = (value + 1).min(max_step);
    //         has_full_kmer_as_predecessor = false;
    //     }
    //
    // }

    todo!("Finish this...");

    sets
}

//
// Code plagiarised from the swbt crates.
// {
//
fn read_lines(path: &PathBuf) -> std::io::Result<Vec<PathBuf>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut paths = Vec::<PathBuf>::new();
    for line in reader.lines() {
        let line = line?;
        paths.push(PathBuf::from(line));
    }
    Ok(paths)
}

struct SeqReader<'a> {
    paths: &'a [PathBuf],
    next_idx: usize,
    current: Option<jseqio::reader::DynamicFastXReader>,
    local_buf: Vec<u8>,
}

impl<'a> SeqReader<'a> {
    fn new(paths: &'a [PathBuf]) -> Self {
        Self {
            paths,
            next_idx: 0,
            current: None,
            local_buf: vec![],
        }
    }
}

impl sbwt::SeqStream for SeqReader<'_> {
    fn stream_next(&mut self) -> Option<&[u8]> {
        loop {
            if let Some(current) = &mut self.current {
                if let Some(rec) = current.read_next().unwrap() {
                    self.local_buf.clear();
                    self.local_buf.extend_from_slice(rec.seq);

                    // note(mk): It's important to reverse the sequence for the suffix array!
                    self.local_buf.reverse();

                    return Some(&self.local_buf);
                } else {
                    self.current = None;
                }
            }

            // Open next file if available
            if self.next_idx < self.paths.len() {
                let path = &self.paths[self.next_idx];
                self.next_idx += 1;
                self.current = Some(jseqio::reader::DynamicFastXReader::from_file(path).unwrap());
            } else {
                return None;
            }
        }
    }
}

impl<'a> Clone for SeqReader<'a> {
    fn clone(&self) -> Self {
        Self {
            paths: self.paths,
            next_idx: 0,
            current: None,
            local_buf: vec![],
        }
    }
}
//
// }
//
