use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};

use pyo3::prelude::*;
use pyo3::types::PyDict;

static INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn install(py: Python<'_>) -> PyResult<()> {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let setup = r#"import builtins
import sys

def _slab_blocked_builtin(*args, **kwargs):
    raise PermissionError("Python plugin ambient file and console access is blocked")

builtins.open = _slab_blocked_builtin
builtins.input = _slab_blocked_builtin
builtins.breakpoint = _slab_blocked_builtin

_SLAB_BLOCKED_AUDIT_EVENTS = {
    'open',
    'os.system',
    'os.remove',
    'os.rename',
    'os.rmdir',
    'os.scandir',
    'os.listdir',
    'shutil.copyfile',
    'shutil.copymode',
    'shutil.copystat',
    'shutil.copytree',
    'shutil.move',
    'shutil.rmtree',
    'socket.__new__',
    'socket.connect',
    'socket.bind',
    'subprocess.Popen',
    'ctypes.dlopen',
    'ctypes.dlsym',
}

def _slab_audit_hook(event, args):
    if event in _SLAB_BLOCKED_AUDIT_EVENTS:
        raise PermissionError('Python plugin ambient operation is blocked: {}'.format(event))

sys.addaudithook(_slab_audit_hook)
"#;
    let code = CString::new(setup).expect("security setup contains no null bytes");
    py.run(&code, Some(&PyDict::new(py)), None)
}
