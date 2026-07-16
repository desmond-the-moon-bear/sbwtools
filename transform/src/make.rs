
use std::ops::IndexMut;

use simple_sds_sbwt::ops::BitVec;

use super::*;

pub const TABLE_SIZE: usize = u8::MAX as usize + 1;
pub const CHAR_TO_INDEX_TABLE: [usize; TABLE_SIZE] = make_char_to_index_table();
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
    CHAR_TO_INDEX_TABLE[byte as usize]
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

    pub fn character(&self, index: usize) -> u8 {
        let char_index = self.get_char_index(index);
        INDEX_TO_CHAR[char_index]
    }

    pub fn lf_step(&self, index: usize) -> usize {
        let char_index = self.get_char_index(index);
        self.counts[char_index] + self.data[char_index].rank(index)
    }

    pub fn inverse_lf_step(&self, index: usize) -> usize {
        assert!(index < self.data[0].len());
        for char_index in (0..self.counts.len()).rev() {
            if index >= self.counts[char_index] {
                let rank_within_count = index - self.counts[char_index];
                let result = self.data[char_index].select(rank_within_count)
                    .expect("The given bit should exist.");
                return result;
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
}

pub struct LcpData<E> {
    data: Vec<u8>,
    len: usize,
    _marker: std::marker::PhantomData<E>,
}

impl<E> LcpData<E> {
    #[inline]
    fn new(data: Vec<u8>) -> Self {
        assert!(data.len().is_multiple_of(size_of::<E>()));
        let len = data.len() / size_of::<E>();
        Self { data, len, _marker: Default::default() }
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

trait Lcp {
    fn get(&self, index: usize) -> usize;
    // fn iter(&self) -> LcpIter;
}

macro_rules! impl_lcp_for_lcp_data {
    ($item:ty) => {
        impl Lcp for LcpData<$item> {
            fn get(&self, index: usize) -> usize {
                assert!(index < self.len);
                let begin = index * size_of::<$item>();
                let end = begin + size_of::<$item>();
                let bytes = self.data[begin..end].try_into().expect("Slice should be correct size.");
                let value = <$item>::from_le_bytes(bytes);
                value as usize
            }
        }
    };
}

impl_lcp_for_lcp_data!(u8);
impl_lcp_for_lcp_data!(u16);
impl_lcp_for_lcp_data!(u32);



