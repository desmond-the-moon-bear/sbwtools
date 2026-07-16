#![allow(unused)]

use clap::{Parser, Subcommand};
use jseqio::reader::*;
use sbwt::SeqStream;

use simple_sds_sbwt::ops::{Rank, Select};
use simple_sds_sbwt::raw_vector::{RawVector, AccessRaw};
use simple_sds_sbwt::bit_vector::BitVector;
use simple_sds_sbwt::serialize::Serialize;

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write, Read};
use std::path::PathBuf;

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
        output.write_all(sequence)?;
        write!(output, "$")?;
    }
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
    let output_byte_count = k_bit_width.div_ceil(u8::BITS) as usize;

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
        match byte[0] {
            b'$' => raw_vectors[0].set_bit(index, true),
            b'A' => raw_vectors[1].set_bit(index, true),
            b'C' => raw_vectors[2].set_bit(index, true),
            b'G' => raw_vectors[3].set_bit(index, true),
            b'T' => raw_vectors[4].set_bit(index, true),
            _ => {}
        };
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
