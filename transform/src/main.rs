use clap::{Parser, Subcommand};
use sbwt::vodbg::count::Counts;
use sbwt::{SbwtIndexVariant, SubsetMatrix, SubsetSeq};
use sbwt::alternative_construction::{Output, preprocessing as preproc};
use preproc::SeqReader;

use simple_sds_sbwt::int_vector::IntVector;
use simple_sds_sbwt::ops::{Access, Vector};
use simple_sds_sbwt::serialize::Serialize;

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

const INDEX_TO_CHAR: &[u8] = b"$ACGT#";

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
    #[command(name = "lcp")]
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
    AsciiToBwt {
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
        bwt_path: PathBuf,

        /// The path to the input (truncated) LCP.
        #[arg(short = 'l')]
        lcp_path: PathBuf,

        /// The value of k.
        #[arg(short = 'k')]
        k: u32,

        /// The path to the output sbwt. Defaults to "./result.bsbwt".
        #[arg(long = "bsbwt-out")]
        sbwt_output_path: Option<PathBuf>,

        /// The path to the output lcs array of the sbwt. Defaults to "./result.lcs".
        #[arg(long = "lcs-out")]
        lcs_output_path: Option<PathBuf>,

        #[arg(short = 'a', long)]
        add_all_dummies: bool,

        /// If all dummies are not added, this argument will be ignored. Path to the output counts.
        #[arg(long = "counts-out")]
        counts_output_path: Option<PathBuf>,
    },
    #[command(name = "verify-sbwt")]
    VerifySbwt {
        #[arg()]
        invariant: PathBuf,

        #[arg()]
        generated: PathBuf,
    },
    #[command(name = "verify-lcs")]
    VerifyLcs {
        #[arg()]
        invariant: PathBuf,

        #[arg()]
        generated: PathBuf,
    },
    #[command(name = "count")]
    Count {
        #[arg(long = "list")]
        list_path: PathBuf,

        #[arg(short)]
        directory_with_sequence_files: Option<PathBuf>,

        #[arg(short)]
        sbwt_path: PathBuf,

        #[arg(short)]
        lcs_path: PathBuf,

        #[arg(short)]
        counts_output_path: PathBuf,
    },
    VerifyCounts {
        #[arg()]
        invariant: PathBuf,

        #[arg()]
        generated: PathBuf,
    },
    #[command(name = "print")]
    Print {
        sbwt: PathBuf,
    }
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
        AsciiToBwt {
            input_path,
            output_path
        } => {
            ascii_to_bwt(input_path, output_path).unwrap();
        },
        Build {
            bwt_path,
            lcp_path,
            k,
            sbwt_output_path,
            lcs_output_path,
            add_all_dummies,
            counts_output_path,
        } => {
            build(bwt_path, lcp_path, sbwt_output_path, lcs_output_path, add_all_dummies, counts_output_path, k).unwrap()
        },
        VerifySbwt { invariant, generated } => { verify_sbwt(invariant, generated).unwrap(); },
        VerifyLcs { invariant, generated } => { verify_lcs(invariant, generated).unwrap(); },
        Count {
            list_path,
            directory_with_sequence_files,
            sbwt_path,
            lcs_path,
            counts_output_path
        } => {
            count(
                list_path,
                directory_with_sequence_files,
                sbwt_path,
                lcs_path,
                counts_output_path
            ).unwrap();
        },
        VerifyCounts { invariant, generated } => { verify_counts(invariant, generated).unwrap(); },
        Print { sbwt } => { print_sbwt(sbwt).unwrap(); },
    };
}

fn concatenate(
    file_list_path: PathBuf,
    directory_with_sequence_files: Option<PathBuf>,
    output_path: Option<PathBuf>,
) -> std::io::Result<()> {
    log::info!("[concatenate] begin");
    let input_sequences = read_lines(&file_list_path)?;
    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.concat"),
    };
    let output_file = File::create(output_path)?;
    if let Some(dir) = directory_with_sequence_files {
        std::env::set_current_dir(dir)?;
    }
    let mut output_writer = BufWriter::new(output_file);
    let mut reader = SeqReader::new(&input_sequences);
    preproc::concatenate_sequences(&mut reader, &mut output_writer)?;
    log::info!("[concatenate] done");
    Ok(())
}

fn truncate_lcp<const BIG_ENDIAN: bool>(
    input_path: PathBuf,
    output_path: Option<PathBuf>,
    k: u32,
) -> std::io::Result<()> {
    log::info!("[truncate_lcp] begin");
    let input_file = File::open(input_path)?;
    let metadata = input_file.metadata()?;
    let length = metadata.len() as usize / size_of::<u64>();
    let mut input_reader = BufReader::new(input_file);

    let output_path = match output_path {
        Some(value) => value,
        None => PathBuf::from("./result.lcpt"),
    };
    let output_file = File::create(output_path)?;
    let mut output_writer = BufWriter::new(output_file);
    let k = k as usize;

    let lcp = preproc::truncate_lcp::<_, BIG_ENDIAN>(&mut input_reader, length, k)?;
    let bytes: Vec<u8> = lcp.into();
    output_writer.write_all(&bytes)?;
    log::info!("[truncate_lcp] done");
    Ok(())
}

fn ascii_to_bwt(
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

    let bwt = preproc::ascii_to_bwt(&mut input_reader, len)?;
    bwt.serialize(&mut output_writer)?;
    log::info!("[tbwt_bit_vectors] done");
    Ok(())
}

fn build(bwt_path: PathBuf, lcp_path: PathBuf, sbwt_output_path: Option<PathBuf>, lcs_output_path: Option<PathBuf>, add_all_dummies: bool, counts_output_path: Option<PathBuf>, k: u32) -> std::io::Result<()> {
    log::info!("[build_remove_redundant_dummies] begin");
    let bwt_file = File::open(bwt_path)?;
    let length = bwt_file.metadata()?.len() as usize;
    let lcp_file = File::open(lcp_path)?;

    let mut bwt_reader = BufReader::new(bwt_file);
    let mut lcp_reader = BufReader::new(lcp_file);

    let sbwt_output_path = match sbwt_output_path {
        Some(value) => value,
        None => PathBuf::from("./result.sbwt"),
    };
    let sbwt_output_file = File::create(sbwt_output_path)?;
    let mut sbwt_writer = BufWriter::new(sbwt_output_file);

    let lcs_output_path = match lcs_output_path {
        Some(value) => value,
        None => PathBuf::from("./result.lcs"),
    };
    let lcs_output_file = File::create(lcs_output_path)?;
    let mut lcs_writer = BufWriter::new(lcs_output_file);

    let build_counts = counts_output_path.is_some();
    let k = k as usize;
    let Output {
        mut sbwt,
        lcs,
        counts
    } = sbwt::alternative_construction::build_from_input::<_, _, SubsetMatrix>(
        &mut bwt_reader,
        &mut lcp_reader,
        length,
        k,
        true,
        add_all_dummies,
        build_counts
    )?;
    
    if let Ok(set_count_string) = std::env::var("RUST_PRINT_SETS") {
        sbwt.build_select();
        let set_count = set_count_string.parse::<usize>().unwrap_or(1);
        let set_count = set_count.min(sbwt.n_sets());

        let mut buf = String::new();
        for i in 0..set_count {
            buf.clear();
            let kmer = sbwt.access_kmer(i);
            let kmer = str::from_utf8(&kmer).unwrap();
            for j in 0..4 {
                if sbwt.sbwt().set_contains(i, j) {
                    buf.push(INDEX_TO_CHAR[j as usize + 1] as char);
                }
            }
            println!("{} {{{}}}", kmer, buf);
        }
    }

    let variant = SbwtIndexVariant::SubsetMatrix(sbwt);
    sbwt::write_sbwt_index_variant(&variant, &mut sbwt_writer)?;

    lcs.unwrap().serialize(&mut lcs_writer)?;

    if build_counts {
        let counts_output_path = counts_output_path.unwrap();
        let counts_file = File::create(counts_output_path)?;
        let mut counts_writer = BufWriter::new(counts_file);
        let counts = counts.unwrap();
        counts.serialize(&mut counts_writer)?;
    }

    Ok(())
}

fn verify_sbwt(invariant: PathBuf, generated: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let invariant_file = File::open(invariant)?;
    let mut invariant_reader = BufReader::new(invariant_file);
    let generated_file = File::open(generated)?;
    let mut generated_reader = BufReader::new(generated_file);

    let sbwt::SbwtIndexVariant::SubsetMatrix(mut invariant) = sbwt::load_sbwt_index_variant(&mut invariant_reader)?;
    let sbwt::SbwtIndexVariant::SubsetMatrix(mut generated) = sbwt::load_sbwt_index_variant(&mut generated_reader)?;
    invariant.build_select();
    generated.build_select();

    if invariant.n_sets() != generated.n_sets() {
        log::info!("ERR: lengths differ: must be {}, was {}", invariant.n_sets(), generated.n_sets());
    } else {
        log::info!("OK: lengths are the same: {}", invariant.n_sets());
    }

    let mut error = false;
    let mut mistake_count = if let Ok(env_value) = std::env::var("RUST_MISTAKE_COUNT") {
        env_value.parse::<usize>().unwrap_or(1)
    } else {
        1
    };

    use sbwt::SubsetSeq;
    for set_index in 0..invariant.n_sets() {
        for i in 0..4 {
            let should_contain_character = invariant.sbwt().set_contains(set_index, i);
            let contains_character = generated.sbwt().set_contains(set_index, i);

            if should_contain_character != contains_character {
                let mut correct_buf = String::new();
                let mut incorrect_buf = String::new();
                let invariant_kmer = invariant.access_kmer(set_index);
                let invariant_kmer = String::from_utf8_lossy(&invariant_kmer);
                let generated_kmer = generated.access_kmer(set_index);
                let generated_kmer = String::from_utf8_lossy(&generated_kmer);
                for j in 0..4 {
                    let should_contain_character = invariant.sbwt().set_contains(set_index, i);
                    if should_contain_character {
                        correct_buf.push(INDEX_TO_CHAR[j + 1] as char);
                    }
                    let contains_character = generated.sbwt().set_contains(set_index, i);
                    if contains_character {
                        incorrect_buf.push(INDEX_TO_CHAR[j + 1] as char);
                    }
                }
                log::info!(
                    "ERR: difference at {}: invariant [{}] {{{}}} | generated [{}] {{{}}}",
                    set_index,
                    invariant_kmer,
                    correct_buf,
                    generated_kmer,
                    incorrect_buf
                );
                mistake_count -= 1;
                error = true;
                if mistake_count < 1 {
                    return Ok(());
                }
                break;
            }
        }
    }

    if !error {
        log::info!("OK: everything is the same");
    }

    Ok(())
}

fn verify_lcs(invariant: PathBuf, generated: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let invariant_file = File::open(invariant)?;
    let mut invariant_reader = BufReader::new(invariant_file);

    let generated_file = File::open(generated)?;
    let mut generated_reader = BufReader::new(generated_file);
    
    let invariant = IntVector::load(&mut invariant_reader)?;
    let generated = IntVector::load(&mut generated_reader)?;

    if invariant.len() != generated.len() {
        log::info!("ERR: lengths differ: must be {}, was {}", invariant.len(), generated.len());
    } else {
        log::info!("OK: lengths are the same: {}", invariant.len());
    }

    for (index, (a, b)) in invariant.iter().zip(generated.iter()).enumerate() {
        if a != b {
            log::info!("ERR: elements at index {} differ: must be {}, was {}", index, a, b);
            return Ok(());
        }
    }

    log::info!("OK: everything is the same");

    Ok(())
}

fn count(
    list_path: PathBuf,
    directory_with_sequence_files: Option<PathBuf>,
    sbwt_path: PathBuf,
    lcs_path: PathBuf,
    counts_output_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let list = read_lines(&list_path)?;
    let sequence_stream = StandardSeqReader::new(&list);
    let sbwt_file = File::open(sbwt_path)?;
    let mut sbwt_reader = BufReader::new(sbwt_file);
    let lcs_file = File::open(lcs_path)?;
    let mut lcs_reader = BufReader::new(lcs_file);
    let counts_file = File::create(counts_output_path)?;
    let mut counts_writer = BufWriter::new(counts_file);

    if let Some(dir) = directory_with_sequence_files {
        std::env::set_current_dir(dir)?;
    }

    let sbwt::SbwtIndexVariant::SubsetMatrix(mut sbwt) = sbwt::load_sbwt_index_variant(&mut sbwt_reader)?;
    sbwt.build_select();
    let lcs = sbwt::LcsArray::load(&mut lcs_reader)?;

    let pnsv = sbwt::vodbg::pnsv::PnsvTuned::new_default(&sbwt, &lcs, 31);
    let mut vodbg = sbwt::vodbg::VoDbg::new(&sbwt, &pnsv);
    let _ = vodbg.build_counts(sequence_stream, true, Counts::DEFAULT_SAMPLE_DISTANCE, 1, 4, Counts::DEFAULT_BATCH_SIZE_IN_BYTES);

    vodbg.counts().unwrap().serialize(&mut counts_writer)?;
    Ok(())
}

fn verify_counts(invariant: PathBuf, generated: PathBuf) -> std::io::Result<()> {
    let invariant_file = File::open(invariant)?;
    let mut invariant_reader = BufReader::new(invariant_file);
    let generated_file = File::open(generated)?;
    let mut generated_reader = BufReader::new(generated_file);

    let invariant = Counts::load(&mut invariant_reader)?;
    let generated = Counts::load(&mut generated_reader)?;

    let mut error = false;
    if invariant.individual_counts.len() != generated.individual_counts.len() {
        log::info!("ERR: individual counts len differs: {} vs {}",
            invariant.individual_counts.len(),
            generated.individual_counts.len()
        );
        error = true;
    }
    if invariant.large_counts.len() != generated.large_counts.len() {
        log::info!("ERR: large counts len differs: {} vs {}",
            invariant.large_counts.len(),
            generated.large_counts.len()
        );
        error = true;
    }

    let mistake_count = if let Ok(env_value) = std::env::var("RUST_MISTAKE_COUNT") {
        env_value.parse::<usize>().unwrap_or(1)
    } else {
        1
    };
    let mut mistake_counter = mistake_count;

    for (index, (a, b)) in invariant.iter().zip(generated.iter()).enumerate() {
        if a != b {
            log::info!("ERR: [{}]: should be {}, was {}", index, a, b);
            mistake_counter -= 1;
            error = true;
            if mistake_counter < 1 {
                break;
            }
        }
    }

    mistake_counter = mistake_count;
    if invariant.sample_information != generated.sample_information {
        log::info!(
            "ERR: sample information differs (len: {} vs {})",
            invariant.sample_information.len(),
            generated.sample_information.len(),
        );
        for (index, (a, b)) in invariant.sample_information.iter().zip(generated.sample_information.iter()).enumerate() {
            if a != b {
                log::info!("ERR: [{}]: should be {:?}, was {:?}", index, a, b);
            }
            mistake_counter -= 1;
            if mistake_counter < 1 {
                break;
            }
        }
        error = true;
    }

    if !error {
        log::info!("OK: everything is the same");
    }

    Ok(())
}

fn print_sbwt(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let sbwt::SbwtIndexVariant::SubsetMatrix(mut sbwt) = sbwt::load_sbwt_index_variant(&mut reader)?;
    sbwt.build_select();

    let set_count = if let Ok(set_count_string) = std::env::var("RUST_PRINT_SETS") {
        let set_count = set_count_string.parse::<usize>().unwrap_or(1);
        set_count.min(sbwt.n_sets())
    } else {
        sbwt.n_sets()
    };

    let mut buf = String::new();
    for set_index in 0..set_count {
        buf.clear();
        for i in 0..4 {
            let should_contain_character = sbwt.sbwt().set_contains(set_index, i);
            if should_contain_character {
                buf.push(INDEX_TO_CHAR[i as usize + 1] as char);
            }
        }
        let kmer = sbwt.access_kmer(set_index);
        let kmer = str::from_utf8(&kmer).unwrap();
        println!("{} {{{}}}", kmer, buf);
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

struct StandardSeqReader<'a> {
    paths: &'a [PathBuf],
    next_idx: usize,
    current: Option<jseqio::reader::DynamicFastXReader>,
    local_buf: Vec<u8>,
}

impl<'a> StandardSeqReader<'a> {
    fn new(paths: &'a [PathBuf]) -> Self {
        Self {
            paths,
            next_idx: 0,
            current: None,
            local_buf: vec![]
        }
    }
}

impl sbwt::SeqStream for StandardSeqReader<'_> {
    fn stream_next(&mut self) -> Option<&[u8]> {
        loop {
            if let Some(current) = &mut self.current {
                if let Some(rec) = current.read_next().unwrap() {
                    self.local_buf.clear();
                    self.local_buf.extend_from_slice(rec.seq);
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

impl<'a> Clone for StandardSeqReader<'a> {
    fn clone(&self) -> Self {
        Self { paths: self.paths, next_idx: 0, current: None, local_buf: vec![] }
    }
}
//
// }
//
