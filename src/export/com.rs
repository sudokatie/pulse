//! COM IUnknown implementation with reference counting

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use super::types::{TResult, TUID, K_RESULT_OK, K_INVALID_ARGUMENT, K_NOT_IMPLEMENTED, tuid_eq, iid};

/// IUnknown/FUnknown vtable - base for all VST3 interfaces
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IUnknownVtable {
    pub query_interface: unsafe extern "system" fn(
        this: *mut c_void,
        iid: *const TUID,
        obj: *mut *mut c_void,
    ) -> TResult,
    pub add_ref: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

/// IUnknown interface object
#[repr(C)]
pub struct IUnknown {
    pub vtable: *const IUnknownVtable,
}

impl IUnknown {
    /// Create a null IUnknown
    pub const fn null() -> Self {
        Self {
            vtable: std::ptr::null(),
        }
    }

    /// Check if the vtable pointer is valid
    pub fn is_valid(&self) -> bool {
        !self.vtable.is_null()
    }
}

/// COM reference with automatic reference counting
pub struct ComRef<T> {
    ptr: *mut T,
}

impl<T> ComRef<T> {
    /// Create from raw pointer (takes ownership)
    pub unsafe fn from_raw(ptr: *mut T) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Get the raw pointer
    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    /// Release ownership and return raw pointer
    pub fn into_raw(self) -> *mut T {
        let ptr = self.ptr;
        std::mem::forget(self);
        ptr
    }
}

impl<T> Clone for ComRef<T> {
    fn clone(&self) -> Self {
        unsafe {
            let unknown = self.ptr as *mut IUnknown;
            if !unknown.is_null() && !(*unknown).vtable.is_null() {
                ((*(*unknown).vtable).add_ref)(unknown as *mut c_void);
            }
        }
        Self { ptr: self.ptr }
    }
}

impl<T> Drop for ComRef<T> {
    fn drop(&mut self) {
        unsafe {
            let unknown = self.ptr as *mut IUnknown;
            if !unknown.is_null() && !(*unknown).vtable.is_null() {
                ((*(*unknown).vtable).release)(unknown as *mut c_void);
            }
        }
    }
}

/// Base COM object with reference counting
#[repr(C)]
pub struct ComObject {
    pub vtable: *const IUnknownVtable,
    pub ref_count: AtomicU32,
}

impl ComObject {
    /// Create a new COM object
    pub fn new(vtable: *const IUnknownVtable) -> Self {
        Self {
            vtable,
            ref_count: AtomicU32::new(1),
        }
    }

    /// Increment reference count
    pub fn add_ref(&self) -> u32 {
        self.ref_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Decrement reference count, returns new count
    pub fn release(&self) -> u32 {
        let prev = self.ref_count.fetch_sub(1, Ordering::SeqCst);
        prev - 1
    }

    /// Get current reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::SeqCst)
    }
}

/// Static IUnknown vtable that returns kNotImplemented for QueryInterface
pub static IUNKNOWN_VTABLE: IUnknownVtable = IUnknownVtable {
    query_interface: iunknown_query_interface,
    add_ref: iunknown_add_ref,
    release: iunknown_release,
};

unsafe extern "system" fn iunknown_query_interface(
    this: *mut c_void,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> TResult {
    if this.is_null() || iid.is_null() || obj.is_null() {
        return K_INVALID_ARGUMENT;
    }

    let requested_iid = &*iid;

    // Only support FUnknown interface
    if tuid_eq(requested_iid, &iid::FUNKNOWN) {
        let com_obj = this as *mut ComObject;
        (*com_obj).add_ref();
        *obj = this;
        return K_RESULT_OK;
    }

    *obj = std::ptr::null_mut();
    K_NOT_IMPLEMENTED
}

unsafe extern "system" fn iunknown_add_ref(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let com_obj = this as *mut ComObject;
    (*com_obj).add_ref()
}

unsafe extern "system" fn iunknown_release(this: *mut c_void) -> u32 {
    if this.is_null() {
        return 0;
    }
    let com_obj = this as *mut ComObject;
    let count = (*com_obj).release();
    if count == 0 {
        // Drop the object
        drop(Box::from_raw(com_obj));
    }
    count
}

/// Trait for types that can be queried via COM QueryInterface
pub trait ComQueryInterface {
    /// Get supported interface IDs and corresponding vtables
    fn supported_interfaces(&self) -> &[(TUID, *const c_void)];

    /// Query for an interface
    fn query_interface(&self, iid: &TUID) -> Option<*mut c_void>;
}

/// Helper to create a QueryInterface implementation
pub fn make_query_interface<T: ComQueryInterface>(
    this: *mut T,
    iid: &TUID,
    obj: *mut *mut c_void,
) -> TResult {
    unsafe {
        if this.is_null() || obj.is_null() {
            return K_INVALID_ARGUMENT;
        }

        // Check FUnknown first
        if tuid_eq(iid, &iid::FUNKNOWN) {
            let com_obj = this as *mut ComObject;
            (*com_obj).add_ref();
            *obj = this as *mut c_void;
            return K_RESULT_OK;
        }

        // Check other interfaces
        if let Some(ptr) = (*this).query_interface(iid) {
            let com_obj = this as *mut ComObject;
            (*com_obj).add_ref();
            *obj = ptr;
            return K_RESULT_OK;
        }

        *obj = std::ptr::null_mut();
        K_NOT_IMPLEMENTED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_com_object_refcount() {
        let obj = ComObject::new(&IUNKNOWN_VTABLE);
        assert_eq!(obj.ref_count(), 1);

        assert_eq!(obj.add_ref(), 2);
        assert_eq!(obj.ref_count(), 2);

        assert_eq!(obj.release(), 1);
        assert_eq!(obj.ref_count(), 1);

        assert_eq!(obj.release(), 0);
    }

    #[test]
    fn test_iunknown_vtable_layout() {
        // Verify vtable is pointer-aligned
        assert_eq!(
            std::mem::align_of::<IUnknownVtable>(),
            std::mem::align_of::<*const c_void>()
        );

        // Verify size is 3 pointers
        assert_eq!(
            std::mem::size_of::<IUnknownVtable>(),
            3 * std::mem::size_of::<*const c_void>()
        );
    }

    #[test]
    fn test_iunknown_query_interface() {
        let obj = Box::new(ComObject::new(&IUNKNOWN_VTABLE));
        let ptr = Box::into_raw(obj);

        unsafe {
            let mut result: *mut c_void = std::ptr::null_mut();

            // Query for FUnknown should succeed
            let status = iunknown_query_interface(
                ptr as *mut c_void,
                &iid::FUNKNOWN,
                &mut result,
            );
            assert_eq!(status, K_RESULT_OK);
            assert!(!result.is_null());

            // Ref count should be 2 now
            assert_eq!((*(ptr)).ref_count(), 2);

            // Release the queried reference
            iunknown_release(result);
            assert_eq!((*(ptr)).ref_count(), 1);

            // Query for unknown interface should fail
            let unknown_iid: TUID = [0xFF; 16];
            result = std::ptr::null_mut();
            let status = iunknown_query_interface(
                ptr as *mut c_void,
                &unknown_iid,
                &mut result,
            );
            assert_eq!(status, K_NOT_IMPLEMENTED);
            assert!(result.is_null());

            // Clean up
            iunknown_release(ptr as *mut c_void);
        }
    }

    #[test]
    fn test_iunknown_null_handling() {
        unsafe {
            let mut result: *mut c_void = std::ptr::null_mut();

            // Null this pointer
            let status = iunknown_query_interface(
                std::ptr::null_mut(),
                &iid::FUNKNOWN,
                &mut result,
            );
            assert_eq!(status, K_INVALID_ARGUMENT);

            // Null iid
            let obj = Box::new(ComObject::new(&IUNKNOWN_VTABLE));
            let ptr = Box::into_raw(obj);
            let status = iunknown_query_interface(
                ptr as *mut c_void,
                std::ptr::null(),
                &mut result,
            );
            assert_eq!(status, K_INVALID_ARGUMENT);

            // Null obj pointer
            let status = iunknown_query_interface(
                ptr as *mut c_void,
                &iid::FUNKNOWN,
                std::ptr::null_mut(),
            );
            assert_eq!(status, K_INVALID_ARGUMENT);

            // Clean up
            drop(Box::from_raw(ptr));
        }
    }

    #[test]
    fn test_add_ref_release_null() {
        unsafe {
            // Should handle null gracefully
            assert_eq!(iunknown_add_ref(std::ptr::null_mut()), 0);
            assert_eq!(iunknown_release(std::ptr::null_mut()), 0);
        }
    }

    #[test]
    fn test_com_ref() {
        let obj = Box::new(ComObject::new(&IUNKNOWN_VTABLE));
        let ptr = Box::into_raw(obj);

        unsafe {
            let com_ref = ComRef::from_raw(ptr).unwrap();
            assert_eq!((*(com_ref.as_ptr())).ref_count(), 1);

            // Clone should increment ref count
            let clone = com_ref.clone();
            assert_eq!((*(com_ref.as_ptr())).ref_count(), 2);

            // Drop clone
            drop(clone);
            assert_eq!((*(com_ref.as_ptr())).ref_count(), 1);

            // into_raw should not decrement
            let raw = com_ref.into_raw();
            assert_eq!((*raw).ref_count(), 1);

            // Clean up
            drop(Box::from_raw(raw));
        }
    }

    #[test]
    fn test_com_ref_null() {
        unsafe {
            let result: Option<ComRef<ComObject>> = ComRef::from_raw(std::ptr::null_mut());
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_iunknown_null_check() {
        let unknown = IUnknown::null();
        assert!(!unknown.is_valid());
        assert!(unknown.vtable.is_null());
    }
}
