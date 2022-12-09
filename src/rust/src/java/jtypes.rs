#![allow(unused_parens)]

use crate::core::signaling;
use crate::webrtc::peer_connection_factory::AudioDevice;

use core::slice;
use std::fmt;

#[repr(C)]
#[derive(Debug)]
pub struct JString {
    len: usize,
    buff: *mut u8,
}

impl JString {
    pub fn to_string(&self) -> String {
        let answer = unsafe { String::from_raw_parts(self.buff, self.len, self.len) };
        answer
    }

/*
    pub fn from_string(src: String) -> Self {
        let string_len = src.len();
        let mut string_bytes = src.as_bytes().as_mut_ptr();
        Self {
            len: string_len,
            buff: string_bytes
        }
    }
*/
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RString<'a> {
    len: usize,
    buff: *const u8,
    phantom: std::marker::PhantomData<&'a u8>,
}

impl<'a> RString<'a> {

    pub fn from_string(src: String) -> Self {
        let string_len = src.len();
        let mut string_bytes = src.as_bytes().as_ptr();
        Self {
            len: string_len,
            buff: string_bytes,
            phantom: std::marker::PhantomData,
        }
    }
}


#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct JArrayByte {
    pub len: usize,
    pub data: [u8; 256],
}

impl fmt::Display for JArrayByte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "JArrayByte with {} bytes at {:?}",
            self.len,
            &(self.data)
        )
    }
}
impl JArrayByte {
    pub fn new(vector: Vec<u8>) -> Self {
        let vlen = vector.len();
        let mut vdata = [0; 256];
        for i in 0..vlen {
            vdata[i] = vector[i];
        }
        JArrayByte {
            len: vlen,
            data: vdata,
        }
    }

    pub fn empty() -> Self {
        let data = [0; 256];
        JArrayByte { len: 0, data: data }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct JArrayByte2D {
    pub len: usize,
    pub data: [u8; 256],
    // pub data: [JArrayByte;25],
}

impl JArrayByte2D {
    pub fn new(vector: Vec<signaling::IceCandidate>) -> Self {
        info!(
            "I have to create a jArrayByte with {} elements",
            vector.len()
        );
        let vlen = vector.len();
        let mut myrows: [JArrayByte; 25] = [JArrayByte::empty(); 25];
        for i in 0..25 {
            if (i < vlen) {
                myrows[i] = JArrayByte::new(vector[i].opaque.clone());
                // myrows[i] = JByteArray::from_data(vector[i].opaque.as_ptr(), vector[i].opaque.len());
                info!("IceVec[{}] = {:?}", i, vector[i].opaque);
            } else {
                // myrows[i] = JByteArray::new(Vec::new());
                myrows[i] = JArrayByte::new(Vec::new());
            }
            info!("Myrow[{}] : {}", i, myrows[i]);
        }
        info!("data at {:?}", myrows);
        JArrayByte2D {
            len: vlen,
            data: [1; 256],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct JByteArray {
    len: usize,
    pub buff: *const u8,
}

impl fmt::Display for JByteArray {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let address = &self.buff;
        write!(f, "jByteArray with {} bytes at {:p}", self.len, self.buff)
    }
}

impl JByteArray {
    pub fn new(vector: Vec<u8>) -> Self {
        let slice = vector.as_slice();
        let buffer = slice.as_ptr();
        JByteArray {
            len: vector.len(),
            buff: buffer,
        }
    }

    pub fn to_vec_u8(&self) -> Vec<u8> {
        let answer = unsafe { slice::from_raw_parts(self.buff, self.len).to_vec() };
        answer
    }

    pub fn empty() -> Self {
        let bar = Vec::new().as_ptr();
        JByteArray { len: 0, buff: bar }
    }

    pub fn from_data(data: *const u8, len: usize) -> Self {
        JByteArray {
            len: len,
            buff: data,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct JByteArray2D {
    pub len: usize,
    pub buff: [JByteArray; 32],
}

impl JByteArray2D {
    pub fn new(vector: Vec<signaling::IceCandidate>) -> Self {
        let vlen = vector.len();
        // let mut myrows = [Opaque::empty(); 25];
        let mut myrows: [JByteArray; 32] = [JByteArray::empty(); 32];
        for i in 0..25 {
            if (i < vlen) {
                myrows[i] =
                    JByteArray::from_data(vector[i].opaque.as_ptr(), vector[i].opaque.len());
            } else {
                myrows[i] = JByteArray::new(Vec::new());
            }
        }
        JByteArray2D {
            len: vlen,
            buff: myrows,
        }
    }
}

#[repr(C)]
struct Buffer {
    data: *mut u8,
    len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TringDevice<'a> {
    index: u32,
    name: RString<'a>,
    unique_id: RString<'a>,
    int_key: RString<'a>,
}

impl<'a> TringDevice<'a> {
    pub fn empty() -> Self {
        let name = RString::from_string("empty".to_string());
        let unique_id = RString::from_string("empty".to_string());
        let int_key = RString::from_string("empty".to_string());
        Self {
            index: 99,
            name: name,
            unique_id: unique_id,
            int_key: int_key
        }
    }

    pub fn from_audio_device(index: u32, src: AudioDevice) -> Self {
        let src_name = RString::from_string(src.name);
        let src_unique_id = RString::from_string(src.unique_id);
        let src_int_key = RString::from_string(src.i18n_key);
        Self {
            index: index,
            name: src_name,
            unique_id: src_unique_id,
            int_key: src_int_key,
        }
    }
    pub fn from_fields(index: u32, src_name:String, src_unique_id:String, src_i18n_key:String) -> Self {
        let src_name = RString::from_string(src_name);
        let src_unique_id = RString::from_string(src_unique_id);
        let src_int_key = RString::from_string(src_i18n_key);
        Self {
            index: index,
            name: src_name,
            unique_id: src_unique_id,
            int_key: src_int_key,
        }
    }
}

