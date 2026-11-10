//! Entry points for the selection bar: where the selection is, and recoloring it.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JDoubleArray};
use jni::sys::{jint, jlong};

use super::common::{BridgeError, document_from_handle, edit};
use crate::android_color::color_from_argb;

/// `static native double[] selectionBounds(long handle)`, as `[minX, minY, maxX, maxY]` in
/// document coordinates, or empty when nothing is selected.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_selectionBounds<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
) -> JDoubleArray<'caller> {
    unowned_env
        .with_env(|env| -> Result<JDoubleArray<'caller>, BridgeError> {
            // SAFETY: `handle` comes from `NativeDocument`, which upholds the module invariants.
            let document = unsafe { document_from_handle(handle) }?;
            let values = document
                .selection_bounds()
                .map(|b| vec![b.mins.x, b.mins.y, b.maxs.x, b.maxs.y])
                .unwrap_or_default();
            let array = env.new_double_array(values.len())?;
            array.set_region(env, 0, &values)?;
            Ok(array)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `static native int recolorSelection(long handle, int argb)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_yx14ng_rnote_viewer_NativeBridge_recolorSelection<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    handle: jlong,
    argb: jint,
) -> jint {
    edit(&mut unowned_env, handle, |document| Ok(document.recolor_selection(color_from_argb(argb))))
}
