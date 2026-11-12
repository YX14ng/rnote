package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Intent
import android.provider.MediaStore
import android.widget.Toast

/**
 * Takes a photo with the system camera into [CameraFileProvider.photoFile]: the camera app
 * does the work, so this app needs no camera permission.
 */
internal object PhotoCapture {
    /** Opens the camera; the answer comes to [Activity.onActivityResult] with [requestCode]. */
    fun start(activity: Activity, requestCode: Int) {
        val file = CameraFileProvider.photoFile(activity)
        file.parentFile?.mkdirs()
        file.delete()
        val uri = CameraFileProvider.photoUri(activity)
        val intent = Intent(MediaStore.ACTION_IMAGE_CAPTURE)
            .putExtra(MediaStore.EXTRA_OUTPUT, uri)
            .addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION)
        // The grant travels with the clip data too, as newer camera apps expect.
        intent.clipData = ClipData.newRawUri("", uri)
        try {
            @Suppress("DEPRECATION")
            activity.startActivityForResult(intent, requestCode)
        } catch (_: ActivityNotFoundException) {
            Toast.makeText(activity, R.string.create_no_camera, Toast.LENGTH_LONG).show()
        }
    }

    /** The photo just taken, or null if there is none (cancelled, or nothing written). */
    fun takenPhoto(activity: Activity): ByteArray? {
        val file = CameraFileProvider.photoFile(activity)
        return file.takeIf { it.length() > 0 }?.readBytes()
    }

    fun clear(activity: Activity) {
        CameraFileProvider.photoFile(activity).delete()
    }
}
