//! VST3 IBStream wrapper for state persistence

use std::ffi::c_void;

use super::component::{IBStream, IBStreamVtable};
use super::com::{ComObject, IUnknownVtable};
use super::types::{
    TResult, K_RESULT_OK, K_INVALID_ARGUMENT, K_NOT_IMPLEMENTED,
    iid, tuid_eq, TUID,
};

/// Seek modes for IBStream
pub const K_SEEK_SET: i32 = 0;
pub const K_SEEK_CUR: i32 = 1;
pub const K_SEEK_END: i32 = 2;

/// Read state data from an IBStream
pub unsafe fn read_state_from_stream(stream: *mut IBStream) -> Result<Vec<u8>, ()> {
    if stream.is_null() || (*stream).vtable.is_null() {
        return Err(());
    }

    let vtable = &*(*stream).vtable;
    let mut data = Vec::new();
    let mut buffer = [0u8; 4096];

    loop {
        let mut bytes_read: i32 = 0;
        let result = (vtable.read)(
            stream,
            buffer.as_mut_ptr() as *mut c_void,
            buffer.len() as i32,
            &mut bytes_read,
        );

        if result != K_RESULT_OK || bytes_read <= 0 {
            break;
        }

        data.extend_from_slice(&buffer[..bytes_read as usize]);
    }

    Ok(data)
}

/// Write state data to an IBStream
pub unsafe fn write_state_to_stream(stream: *mut IBStream, data: &[u8]) -> Result<(), ()> {
    if stream.is_null() || (*stream).vtable.is_null() {
        return Err(());
    }

    let vtable = &*(*stream).vtable;
    let mut offset = 0;

    while offset < data.len() {
        let chunk = &data[offset..];
        let to_write = chunk.len().min(4096) as i32;
        let mut bytes_written: i32 = 0;

        let result = (vtable.write)(
            stream,
            chunk.as_ptr() as *const c_void,
            to_write,
            &mut bytes_written,
        );

        if result != K_RESULT_OK || bytes_written <= 0 {
            return Err(());
        }

        offset += bytes_written as usize;
    }

    Ok(())
}

/// Memory-backed IBStream implementation for testing and internal use
#[repr(C)]
pub struct MemoryStream {
    pub com: ComObject,
    data: Vec<u8>,
    position: usize,
}

impl MemoryStream {
    /// Create a new empty memory stream
    pub fn new() -> Box<Self> {
        Box::new(Self {
            com: ComObject::new(&MEMORY_STREAM_VTABLE as *const _ as *const IUnknownVtable),
            data: Vec::new(),
            position: 0,
        })
    }

    /// Create a memory stream with initial data
    pub fn with_data(data: Vec<u8>) -> Box<Self> {
        Box::new(Self {
            com: ComObject::new(&MEMORY_STREAM_VTABLE as *const _ as *const IUnknownVtable),
            data,
            position: 0,
        })
    }

    /// Get the data as a slice
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Take ownership of the data
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    /// Get the underlying data as a mutable reference
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    /// Get current position
    pub fn position(&self) -> usize {
        self.position
    }

    /// Get as IBStream pointer
    pub fn as_ibstream(&mut self) -> *mut IBStream {
        self as *mut Self as *mut IBStream
    }
}

impl Default for MemoryStream {
    fn default() -> Self {
        Self {
            com: ComObject::new(&MEMORY_STREAM_VTABLE as *const _ as *const IUnknownVtable),
            data: Vec::new(),
            position: 0,
        }
    }
}

// IBStream vtable for MemoryStream
static MEMORY_STREAM_VTABLE: IBStreamVtable = IBStreamVtable {
    unknown: IUnknownVtable {
        query_interface: memory_stream_query_interface,
        add_ref: memory_stream_add_ref,
        release: memory_stream_release,
    },
    read: memory_stream_read,
    write: memory_stream_write,
    seek: memory_stream_seek,
    tell: memory_stream_tell,
};

unsafe extern "system" fn memory_stream_query_interface(
    this: *mut c_void,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || iid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let requested_iid = &*iid;

    if tuid_eq(requested_iid, &iid::FUNKNOWN) {
        let stream = this as *mut MemoryStream;
        (*stream).com.add_ref();
        *obj = this;
        return K_RESULT_OK;
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn memory_stream_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let stream = this as *mut MemoryStream;
    (*stream).com.add_ref()
}

unsafe extern "system" fn memory_stream_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let stream = this as *mut MemoryStream;
    let count = (*stream).com.release();
    if count == 0 {
        drop(Box::from_raw(stream));
    }
    count
}

unsafe extern "system" fn memory_stream_read(
    this: *mut IBStream,
    buffer: *mut c_void,
    num_bytes: i32,
    num_bytes_read: *mut i32,
) -> TResult {
    if this.is_null() || buffer.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let stream = this as *mut MemoryStream;
    let available = (*stream).data.len().saturating_sub((*stream).position);
    let to_read = (num_bytes as usize).min(available);

    if to_read > 0 {
        let src = &(&(*stream).data)[(*stream).position..(*stream).position + to_read];
        std::ptr::copy_nonoverlapping(src.as_ptr(), buffer as *mut u8, to_read);
        (*stream).position += to_read;
    }

    if !num_bytes_read.is_null() {
        *num_bytes_read = to_read as i32;
    }

    K_RESULT_OK
}

unsafe extern "system" fn memory_stream_write(
    this: *mut IBStream,
    buffer: *const c_void,
    num_bytes: i32,
    num_bytes_written: *mut i32,
) -> TResult {
    if this.is_null() || buffer.is_null() || num_bytes < 0 {
        return K_INVALID_ARGUMENT;
    }

    let stream = this as *mut MemoryStream;
    let bytes_to_write = num_bytes as usize;

    // Ensure we have enough space
    let end_pos = (*stream).position + bytes_to_write;
    if end_pos > (*stream).data.len() {
        (*stream).data.resize(end_pos, 0);
    }

    // Copy data
    let src = std::slice::from_raw_parts(buffer as *const u8, bytes_to_write);
    (&mut (*stream).data)[(*stream).position..end_pos].copy_from_slice(src);
    (*stream).position = end_pos;

    if !num_bytes_written.is_null() {
        *num_bytes_written = bytes_to_write as i32;
    }

    K_RESULT_OK
}

unsafe extern "system" fn memory_stream_seek(
    this: *mut IBStream,
    pos: i64,
    mode: i32,
    result: *mut i64,
) -> TResult {
    if this.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let stream = this as *mut MemoryStream;
    let new_pos = match mode {
        K_SEEK_SET => pos as usize,
        K_SEEK_CUR => {
            if pos < 0 {
                (*stream).position.saturating_sub((-pos) as usize)
            } else {
                (*stream).position.saturating_add(pos as usize)
            }
        }
        K_SEEK_END => {
            if pos < 0 {
                (*stream).data.len().saturating_sub((-pos) as usize)
            } else {
                (*stream).data.len().saturating_add(pos as usize)
            }
        }
        _ => return K_INVALID_ARGUMENT,
    };

    (*stream).position = new_pos.min((*stream).data.len());

    if !result.is_null() {
        *result = (*stream).position as i64;
    }

    K_RESULT_OK
}

unsafe extern "system" fn memory_stream_tell(
    this: *mut IBStream,
    pos: *mut i64,
) -> TResult {
    if this.is_null() || pos.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let stream = this as *const MemoryStream;
    *pos = (*stream).position as i64;

    K_RESULT_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stream_new() {
        let stream = MemoryStream::new();
        assert_eq!(stream.data().len(), 0);
        assert_eq!(stream.position(), 0);
    }

    #[test]
    fn test_memory_stream_with_data() {
        let data = vec![1, 2, 3, 4, 5];
        let stream = MemoryStream::with_data(data.clone());
        assert_eq!(stream.data(), &data);
        assert_eq!(stream.position(), 0);
    }

    #[test]
    fn test_memory_stream_write() {
        let mut stream = MemoryStream::new();
        let ptr = stream.as_ibstream();

        unsafe {
            let data = [1u8, 2, 3, 4, 5];
            let mut bytes_written: i32 = 0;

            let result = memory_stream_write(
                ptr,
                data.as_ptr() as *const c_void,
                data.len() as i32,
                &mut bytes_written,
            );

            assert_eq!(result, K_RESULT_OK);
            assert_eq!(bytes_written, 5);
            assert_eq!(stream.data(), &data);
        }
    }

    #[test]
    fn test_memory_stream_read() {
        let data = vec![1, 2, 3, 4, 5];
        let mut stream = MemoryStream::with_data(data);
        let ptr = stream.as_ibstream();

        unsafe {
            let mut buffer = [0u8; 10];
            let mut bytes_read: i32 = 0;

            let result = memory_stream_read(
                ptr,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() as i32,
                &mut bytes_read,
            );

            assert_eq!(result, K_RESULT_OK);
            assert_eq!(bytes_read, 5);
            assert_eq!(&buffer[..5], &[1, 2, 3, 4, 5]);
        }
    }

    #[test]
    fn test_memory_stream_seek() {
        let data = vec![1, 2, 3, 4, 5];
        let mut stream = MemoryStream::with_data(data);
        let ptr = stream.as_ibstream();

        unsafe {
            let mut result_pos: i64 = 0;

            // Seek from start
            let result = memory_stream_seek(ptr, 2, K_SEEK_SET, &mut result_pos);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(result_pos, 2);

            // Seek from current
            let result = memory_stream_seek(ptr, 1, K_SEEK_CUR, &mut result_pos);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(result_pos, 3);

            // Seek from end
            let result = memory_stream_seek(ptr, -2, K_SEEK_END, &mut result_pos);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(result_pos, 3);
        }
    }

    #[test]
    fn test_memory_stream_tell() {
        let mut stream = MemoryStream::new();
        let ptr = stream.as_ibstream();

        unsafe {
            let mut pos: i64 = -1;

            let result = memory_stream_tell(ptr, &mut pos);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(pos, 0);

            // Write some data to advance position
            let data = [1u8, 2, 3];
            memory_stream_write(
                ptr,
                data.as_ptr() as *const c_void,
                3,
                std::ptr::null_mut(),
            );

            let result = memory_stream_tell(ptr, &mut pos);
            assert_eq!(result, K_RESULT_OK);
            assert_eq!(pos, 3);
        }
    }

    #[test]
    fn test_read_write_state() {
        let mut stream = MemoryStream::new();
        let ptr = stream.as_ibstream();

        let original_data = vec![10, 20, 30, 40, 50];

        unsafe {
            // Write state
            let result = write_state_to_stream(ptr, &original_data);
            assert!(result.is_ok());

            // Reset position
            memory_stream_seek(ptr, 0, K_SEEK_SET, std::ptr::null_mut());

            // Read state
            let read_data = read_state_from_stream(ptr);
            assert!(read_data.is_ok());
            assert_eq!(read_data.unwrap(), original_data);
        }
    }

    #[test]
    fn test_memory_stream_write_extends_buffer() {
        let mut stream = MemoryStream::new();
        let ptr = stream.as_ibstream();

        unsafe {
            // Write first chunk
            let data1 = [1u8, 2, 3];
            memory_stream_write(
                ptr,
                data1.as_ptr() as *const c_void,
                3,
                std::ptr::null_mut(),
            );

            // Write second chunk
            let data2 = [4u8, 5, 6];
            memory_stream_write(
                ptr,
                data2.as_ptr() as *const c_void,
                3,
                std::ptr::null_mut(),
            );

            assert_eq!(stream.data(), &[1, 2, 3, 4, 5, 6]);
        }
    }

    #[test]
    fn test_memory_stream_read_at_end() {
        let data = vec![1, 2, 3];
        let mut stream = MemoryStream::with_data(data);
        let ptr = stream.as_ibstream();

        unsafe {
            // Seek to end
            memory_stream_seek(ptr, 0, K_SEEK_END, std::ptr::null_mut());

            // Try to read
            let mut buffer = [0u8; 10];
            let mut bytes_read: i32 = 0;

            let result = memory_stream_read(
                ptr,
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len() as i32,
                &mut bytes_read,
            );

            assert_eq!(result, K_RESULT_OK);
            assert_eq!(bytes_read, 0);
        }
    }

    #[test]
    fn test_memory_stream_partial_read() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut stream = MemoryStream::with_data(data);
        let ptr = stream.as_ibstream();

        unsafe {
            let mut buffer = [0u8; 3];
            let mut bytes_read: i32 = 0;

            // First read
            memory_stream_read(
                ptr,
                buffer.as_mut_ptr() as *mut c_void,
                3,
                &mut bytes_read,
            );
            assert_eq!(bytes_read, 3);
            assert_eq!(&buffer, &[1, 2, 3]);

            // Second read
            memory_stream_read(
                ptr,
                buffer.as_mut_ptr() as *mut c_void,
                3,
                &mut bytes_read,
            );
            assert_eq!(bytes_read, 3);
            assert_eq!(&buffer, &[4, 5, 6]);

            // Third read (partial)
            memory_stream_read(
                ptr,
                buffer.as_mut_ptr() as *mut c_void,
                3,
                &mut bytes_read,
            );
            assert_eq!(bytes_read, 2);
        }
    }

    #[test]
    fn test_null_pointer_handling() {
        unsafe {
            // read with null stream
            let result = read_state_from_stream(std::ptr::null_mut());
            assert!(result.is_err());

            // write with null stream
            let result = write_state_to_stream(std::ptr::null_mut(), &[1, 2, 3]);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_memory_stream_ref_counting() {
        let stream = MemoryStream::new();
        let ptr = Box::into_raw(stream);

        unsafe {
            assert_eq!((*(ptr)).com.ref_count(), 1);

            memory_stream_add_ref(ptr as *mut c_void);
            assert_eq!((*(ptr)).com.ref_count(), 2);

            memory_stream_release(ptr as *mut c_void);
            assert_eq!((*(ptr)).com.ref_count(), 1);

            // Final release
            memory_stream_release(ptr as *mut c_void);
            // Object is now freed
        }
    }

    #[test]
    fn test_into_data() {
        let original = vec![1, 2, 3, 4, 5];
        let stream = MemoryStream::with_data(original.clone());
        let result = stream.into_data();
        assert_eq!(result, original);
    }
}
