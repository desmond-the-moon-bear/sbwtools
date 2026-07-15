#![allow(unused)]

use jseqio::reader::*;
use std::path::PathBuf;

fn main() {
    println!("._.");
}

//
// Code plagiarised from the swbt crates.
// {
//
trait SeqStream{
    fn stream_next(&mut self) -> Option<&[u8]>;
}

fn read_lines(path: &PathBuf) -> Vec<PathBuf> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).unwrap();
    let reader = std::io::BufReader::new(file);
    let mut paths = Vec::<PathBuf>::new();
    for line in reader.lines() {
        let line = line.unwrap();
        paths.push(PathBuf::from(line));
    }
    paths
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
            local_buf: vec![]
        }
    }
}

impl SeqStream for SeqReader<'_> {
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
        Self { paths: self.paths, next_idx: 0, current: None, local_buf: vec![] }
    }
}
//
// }
//
