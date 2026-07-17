
use std::ops::IndexMut;

use simple_sds_sbwt::ops::BitVec;

use super::*;

pub const TABLE_SIZE: usize = u8::MAX as usize + 1;
pub const CHAR_TO_INDEX: [usize; TABLE_SIZE] = make_char_to_index_table();
pub const INDEX_TO_CHAR: [u8; 5] = [b'$', b'A', b'C', b'G', b'T'];

const fn make_char_to_index_table() -> [usize; TABLE_SIZE] {
    let mut table = [u8::MAX as usize; TABLE_SIZE];
    table[b'$' as usize] = 0; 
    table[b'A' as usize] = 1; 
    table[b'C' as usize] = 2; 
    table[b'G' as usize] = 3; 
    table[b'T' as usize] = 4; 
    table
}

#[inline(always)]
pub fn char_index(byte: u8) -> usize {
    CHAR_TO_INDEX[byte as usize]
}

pub struct Bwt {
    data: [BitVector; 5],
    counts: [usize; 5],
}

impl Bwt {
    pub fn new(data: [BitVector; 5]) -> Self {
        let mut counts = [0_usize; 5];
        for i in 1..5 {
            counts[i] = counts[i - 1] + data[i - 1].count_ones();
        }
        Self {
            data,
            counts
        }
    }

    #[inline]
    pub fn get_char_index(&self, index: usize) -> usize {
        assert!(index < self.data[0].len());
        for char_index in 0..self.data.len() {
            if self.data[char_index].get(index) {
                return char_index;
            }
        }
        unreachable!("The character at the index should have a corresponding bitvector in the BWT.");
    }

    #[inline]
    pub fn character(&self, index: usize) -> u8 {
        let char_index = self.get_char_index(index);
        INDEX_TO_CHAR[char_index]
    }

    #[inline]
    pub fn lf_step(&self, index: usize) -> (usize, u8) {
        let char_index = self.get_char_index(index);
        let order = self.counts[char_index] + self.data[char_index].rank(index);
        let character = INDEX_TO_CHAR[char_index];
        (order, character)
    }

    #[inline]
    pub fn inverse_lf_step(&self, index: usize) -> usize {
        assert!(index < self.data[0].len());
        for char_index in (0..self.counts.len()).rev() {
            if index >= self.counts[char_index] {
                let rank_within_count = index - self.counts[char_index];
                let order = self.data[char_index].select(rank_within_count)
                    .expect("The given bit should exist.");
                return order;
            }
        }
        unreachable!()
    }

    pub fn load<R: std::io::Read>(input: &mut R) -> std::io::Result<Self> {
        let data = [
            BitVector::load(input)?,
            BitVector::load(input)?,
            BitVector::load(input)?,
            BitVector::load(input)?,
            BitVector::load(input)?,
        ];
        let result = Self::new(data);
        Ok(result)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data[0].len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data[0].len() == 0
    }
}

pub struct Lcp {
    data: Vec<u8>,
    len: usize,
    width: usize,
    offset: usize,
}

impl Lcp {
    #[inline]
    pub fn new<E>(data: Vec<u8>) -> Self {
        assert!(size_of::<E>() <= size_of::<usize>());
        assert!(data.len().is_multiple_of(size_of::<E>()));
        let len = data.len() / size_of::<E>();
        Self {
            data,
            len,
            width: size_of::<E>(),
            offset: 0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Iterator for Lcp {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }
        let end = self.offset + self.width;
        let mut bytes: [u8; 8] = [0_u8; 8];
        let src = &self.data[self.offset..end];
        bytes[0..self.width].copy_from_slice(src);
        let value = usize::from_le_bytes(bytes);
        let mut v: usize = 0;
        self.offset = end;
        Some(value)
    }
}

impl From<Lcp> for Vec<u8> {
    fn from(value: Lcp) -> Self {
        value.data
    }
}

