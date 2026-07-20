#![allow(unused)]

mod make;

use clap::{Parser, Subcommand, ValueEnum};
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

        #[arg(value_enum, short = 's', default_value_t=Separator::Dollar)]
        separator: Separator,
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
    },
    /// Convert the sets from a sequence of bytes to a serialized SubsetMatrix variant of the SBWT.
    #[command(name = "re")]
    Reserialize {
        /// Path to a ".bsbwt" file.
        #[arg(short = 'i')]
        bsbwt_path: PathBuf,

        /// The path to the output. Defaults to "./result.sbwt".
        #[arg(short = 'o')]
        output_path: Option<PathBuf>,
    },
    /// Verify that the sets constructed from the LCP array and the BWT contain the same
    /// information as an already generated SBWT data structure.
    #[command(name = "verify")]
    Verify {
        #[arg(short = 's')]
        sbwt_path: PathBuf, 

        #[arg(long = "bsbwt")]
        bsbwt_path: PathBuf,
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[value()]
enum Separator {
    #[value()]
    #[default]
    Dollar,
    #[value()]
    Null,
}

fn main() {
    env_logger::init();
    let Cli { command } = Cli::parse();
    use Commands::*;
    match command {
        Concatenate {
            file_list_path,
            directory_with_sequence_files,
            output_path,
            separator,
        } => {
            concatenate(file_list_path, directory_with_sequence_files, output_path, separator).unwrap();
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
        Reserialize { bsbwt_path, output_path } => {},
        Verify { sbwt_path, bsbwt_path } => { verify(sbwt_path, bsbwt_path).unwrap() }
    };
}

fn concatenate(
    file_list_path: PathBuf,
    directory_with_sequence_files: Option<PathBuf>,
    output_path: Option<PathBuf>,
    separator: Separator,
) -> std::io::Result<()> {
    log::info!("[concatenate] begin");
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
    use Separator::*;
    match separator {
        Dollar => { write_concatenation(sequence_reader, &mut output_writer, '$')?; },
        Null => { write_concatenation_gsa(sequence_reader, &mut output_writer)?; },
    }
    log::info!("[concatenate] done");
    Ok(())
}

pub fn write_concatenation<SS: SeqStream + Send, W: std::io::Write>(mut stream: SS, output: &mut W, separator: char) -> std::io::Result<()> {
    write!(output, "{}", separator)?;
    while let Some(sequence) = stream.stream_next() {
        write!(output, "{}", separator)?;
        output.write_all(sequence)?;
    }
    write!(output, "{}", separator)?;
    Ok(())
}

pub fn write_concatenation_gsa<SS: SeqStream + Send, W: std::io::Write>(mut stream: SS, output: &mut W) -> std::io::Result<()> {
    write!(output, "\0")?;
    while let Some(sequence) = stream.stream_next() {
        output.write_all(sequence)?;
        write!(output, "\0")?;
    }
    Ok(())
}

fn truncate_lcp<const BIG_ENDIAN: bool>(
    input_path: PathBuf,
    output_path: Option<PathBuf>,
    max_k: u32,
) -> std::io::Result<()> {
    log::info!("[truncate_lcp] begin");
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
    log::info!("[truncate_lcp] done");
    Ok(())
}

fn bwt_bit_vectors(
    input_path: PathBuf,
    output_path: Option<PathBuf>,
) -> std::io::Result<()> {
    log::info!("[tbwt_bit_vectors] begin");
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
    log::info!("[tbwt_bit_vectors] done");
    Ok(())
}

fn build(bwtb_path: PathBuf, lcpt_path: PathBuf, output_path: Option<PathBuf>, k: u32) -> std::io::Result<()> {
    log::info!("[build] begin");
    let bwtb_file = File::open(bwtb_path)?;
    let mut lcpt_file = File::open(lcpt_path)?;

    let mut bwtb_reader = BufReader::new(bwtb_file);

    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.bsbwt"),
    };
    let mut output_file = File::create(output_path)?;

    log::info!("[build] loading bwt");
    let bwt = Bwt::load(&mut bwtb_reader)?;

    log::info!("[build] loading lcpt");
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

    let (shorter_than_k, equal_to_k, regions, k_regions) = calculate_auxiliary_bitvectors(&bwt, &mut lcp, k);
    let width = lcp.width();
    drop(lcp);

    let (keep_suffix, keep_letter) = calculate_dummy_marks(
        &bwt, k, &regions, &k_regions, &shorter_than_k, &equal_to_k
    );
    drop(equal_to_k);

    let sets = build_sets(&bwt, width, &regions, &k_regions, &shorter_than_k, &keep_suffix, &keep_letter);
    output_file.write_all(&sets);
    log::info!("[build] done");
    Ok(())
}

fn calculate_auxiliary_bitvectors(bwt: &Bwt, lcp: &mut Lcp, k: usize) -> (BitVector, RawVector, BitVector, RawVector) {
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
    // A big region can be further divided into k-regions.

    log::info!("[calculate_auxiliary_bitvectors] begin");
    let len = bwt.len();
    let mut shorter_than_k = RawVector::with_len(len, false);
    let mut equal_to_k     = RawVector::with_len(len, false);
    let mut regions        = RawVector::with_len(len, false);
    let mut k_regions      = RawVector::with_len(len, false);
    let mut order = 0;
    let mut current_length = 0;
    for _ in 0..len {
        let (next_order, character) = bwt.lf_step(order);
        order = next_order;
        if character == b'$' {
            current_length = 0;
        } else {
            current_length += 1;
            if current_length < k {
                shorter_than_k.set_bit(order, true);
            } else if current_length == k {
                equal_to_k.set_bit(order, true);
            } else {
                current_length = k;
            }
        }
        let lcp_value = lcp.get(order);
        if lcp_value < current_length {
            k_regions.set_bit(order, true);
            if current_length < k || lcp_value < k - 1 {
                regions.set_bit(order, true);
            }
        }
    }

    regions.set_bit(0, true);
    k_regions.set_bit(0, true);

    log::info!("[calculate_auxiliary_bitvectors] rank for shorter than k k-mers bitvector");
    let mut shorter_than_k = BitVector::from(shorter_than_k);
    shorter_than_k.enable_rank();
    log::info!("[calculate_auxiliary_bitvectors] rank and select for regions bitvector");
    let mut regions = BitVector::from(regions);
    regions.enable_rank();
    regions.enable_select();
    (shorter_than_k, equal_to_k, regions, k_regions)
}

fn calculate_dummy_marks(
    bwt: &Bwt,
    k: usize,
    regions: &BitVector,
    k_regions: &RawVector,
    shorter_than_k: &BitVector,
    equal_to_k: &RawVector,
) -> (RawVector, RawVector) {
    log::info!("[calculate_dummy_marks] begin");
    let len = bwt.len();
    let mut keep_suffix = RawVector::with_len(len, false);
    let mut keep_letter = RawVector::with_len(len, false);

    let mut start = bwt.data[char_index(b'$')].count_ones();
    let mut has_full_kmer_as_predecessor = false;
    for index in start..bwt.len() {
        if k_regions.bit(index) {
            has_full_kmer_as_predecessor = false;
        }

        if equal_to_k.bit(index) {
            let predecessor = bwt.inverse_lf_step(index);
            // If we haven't found a full k-mer as a predecessor for this k-region, search for it.
            if !has_full_kmer_as_predecessor {
                has_full_kmer_as_predecessor |= has_full_kmer_predecessor(
                    predecessor, bwt, regions, shorter_than_k
                );
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
    }

    (keep_suffix, keep_letter)
}

fn has_full_kmer_predecessor(
    predecessor: usize,
    bwt: &Bwt,
    regions: &BitVector, 
    shorter_than_k: &BitVector
) -> bool {
    let range_start = predecessor;
    let one_index = regions.rank(range_start + 1);
    let range_end = if one_index == regions.count_ones() {
        bwt.len()
    } else {
        // There is at least one 1 after the current position.
        regions.select(one_index).unwrap()
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
    width: usize,
    regions: &BitVector,
    k_regions: &RawVector,
    shorter_than_k: &BitVector,
    keep_suffix: &RawVector,
    keep_letter: &RawVector,
) -> Vec<u8> {
    log::info!("[build_sets] begin");
    const FULL_SET: u8 = 0b00001111;

    let mut sets = Vec::<u8>::with_capacity(bwt.len());

    let _ = width;
    // todo(mk): construct the lcs array of the SBWT as well.
    // let mut lcs_data = Vec::<u8>::with_capacity(bwt.len());
    // let mut lcs = Lcp::new_with_width(lcs_data, width);

    let mut current_set: u8 = 0;

    let mut separator_count = bwt.data[char_index(b'$')].count_ones();

    for index in 0..separator_count {
        if keep_suffix.bit(index) {
            current_set = include_letter(bwt, index, current_set);
            if current_set == FULL_SET {
                break;
            }
        }
    }
    sets.push(current_set);
    log::info!("[build_sets] done with $ region");

    current_set = 0;
    let mut include_dummy_kmer = false;
    let mut has_dummy_kmer     = false;
    let mut k_region_count = 0;
    for index in separator_count..bwt.len() {
        if regions.get(index) {
            if has_dummy_kmer && !include_dummy_kmer {
                k_region_count -= 1;
            }
            while k_region_count > 0 {
                sets.push(current_set);
                current_set = 0;
                k_region_count -= 1;
            }

            current_set = 0;
            has_dummy_kmer = false;
            include_dummy_kmer = false;
            k_region_count = 0;
        }

        if k_regions.bit(index) {
            k_region_count += 1;
        }

        if shorter_than_k.get(index) {
            has_dummy_kmer = true;
            if keep_suffix.bit(index) {
                include_dummy_kmer = true;
                current_set = include_letter(bwt, index, current_set);
            }
            if keep_letter.bit(index) {
                current_set = include_letter(bwt, index, current_set);
            }
        } else {
            current_set = include_letter(bwt, index, current_set);
        }
    }

    if has_dummy_kmer && !include_dummy_kmer {
        k_region_count -= 1;
    }
    while k_region_count > 0 {
        sets.push(current_set);
        current_set = 0;
        k_region_count -= 1;
    }
    log::info!("[build_sets] done with other regions");

    sets
}

#[inline]
fn include_letter(bwt: &Bwt, index: usize, current_set: u8) -> u8 {
    (1 << (bwt.get_char_index(index) - 1)) as u8 | current_set
}

fn reserialize(bsbwt_path: PathBuf, output_path: Option<PathBuf>) -> std::io::Result<()> {
    todo!("Needs more data structures.");

    log::info!("[reserialize] begin");

    let mut bsbwt_file = File::open(bsbwt_path)?;
    let metadata = bsbwt_file.metadata()?;
    let len = metadata.len() as usize;

    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.sbwt"),
    };

    let output_file = File::create(output_path)?;
    let mut output_writer = BufWriter::new(output_file);

    let mut sets = Vec::<u8>::with_capacity(len);
    bsbwt_file.read_to_end(&mut sets);

    let mut raw_vectors = [
        RawVector::with_len(len, false), // A
        RawVector::with_len(len, false), // C
        RawVector::with_len(len, false), // G
        RawVector::with_len(len, false), // T
    ];

    for (set_index, set) in sets.into_iter().enumerate() {
        for (row_index, row) in raw_vectors.iter_mut().enumerate() {
            if set & (1 << row_index) == 0 {
                continue;
            }
            row.set_bit(set_index, true);
        }
    }

    const VARIANT_STRING: &[u8] = b"SubsetMatrix";
    output_writer.write_all(&(VARIANT_STRING.len() as u64).to_le_bytes())?;
    output_writer.write_all(VARIANT_STRING);

    log::info!("[reserialize] done");
    Ok(())
}

fn verify(sbwt_path: PathBuf, bsbwt_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let sbwt_file = File::open(sbwt_path)?;
    let mut sbwt_reader = BufReader::new(sbwt_file);

    let bsbwt_file = File::open(bsbwt_path)?;
    let bsbwt_metadata = bsbwt_file.metadata()?;
    let bsbwt_len = bsbwt_metadata.len() as usize;
    let mut bsbwt = Vec::<u8>::with_capacity(bsbwt_len);
    
    let sbwt::SbwtIndexVariant::SubsetMatrix(matrix) = sbwt::load_sbwt_index_variant(&mut sbwt_reader)?;
    if matrix.n_sets() != bsbwt_len {
        log::info!("FAIL: lengths differ; stopping verification");
        return Ok(());
    }

    use sbwt::SubsetSeq;
    for (set_index, set) in bsbwt.into_iter().enumerate() {
        for i in 0..4 {
            let should_contain_character = matrix.sbwt.set_contains(set_index, i);
            let set_contains_character = (set & (1 << i)) != 0;
            if should_contain_character != set_contains_character {
                log::info!("FAIL: set {} does not match", set_index);
                return Ok(());
            }
        }
    }

    log::info!("OK");

    Ok(())
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
