package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.util.Log
import com.google.android.gms.auth.api.identity.AuthorizationRequest
import com.google.android.gms.auth.api.identity.AuthorizationResult
import com.google.android.gms.auth.api.identity.ClearTokenRequest
import com.google.android.gms.auth.api.identity.Identity
import com.google.android.gms.common.api.ApiException
import com.google.android.gms.common.api.CommonStatusCodes
import com.google.android.gms.common.api.Scope

/** What asking Google for an access token gave. */
internal sealed interface TokenResult {
    data class Token(val value: String) : TokenResult {
        // Never in logs.
        override fun toString() = "Token(…)"
    }

    /** The user has to pick the account or agree: only shown from the sync button. */
    data class NeedsUser(val intent: PendingIntent) : TokenResult

    /** This build isn't registered in Google: signed with another key than the stable one. */
    data object NotRegistered : TokenResult

    data class Failed(val error: Throwable) : TokenResult
}

/**
 * The access token for the Drive sync, from Google's Authorization API: the app is
 * identified by its package and the SHA-1 of its signing key, so there is no client id here.
 * Callbacks come on the main thread.
 */
internal object DriveAuth {
    /** Only the files the app creates. */
    const val SCOPE = "https://www.googleapis.com/auth/drive.file"

    private const val TAG = "RnoteSync"

    /** Asks for the token without showing anything; [TokenResult.NeedsUser] if that's not enough. */
    fun token(context: Context, done: (TokenResult) -> Unit) {
        Identity.getAuthorizationClient(context).authorize(request())
            .addOnSuccessListener { result -> done(resultOf(result)) }
            .addOnFailureListener { error -> done(failureOf(error)) }
    }

    /**
     * Signs in: shows Google's account picker and consent if needed, which come back to
     * [activity]'s `onActivityResult` with [requestCode], to pass to [finishSignIn]. [done]
     * only runs when nothing had to be shown, or on failure.
     */
    fun startSignIn(activity: Activity, requestCode: Int, done: (TokenResult) -> Unit) {
        token(activity) { result ->
            if (result is TokenResult.NeedsUser) {
                try {
                    activity.startIntentSenderForResult(result.intent.intentSender, requestCode, null, 0, 0, 0)
                } catch (error: Exception) {
                    done(TokenResult.Failed(error))
                }
            } else {
                done(result)
            }
        }
    }

    /** The token from Google's screens, given the `onActivityResult` data of [startSignIn]. */
    fun finishSignIn(activity: Activity, data: Intent?): TokenResult =
        try {
            resultOf(Identity.getAuthorizationClient(activity).getAuthorizationResultFromIntent(data))
        } catch (error: Exception) {
            failureOf(error)
        }

    /** Forgets [token] in the device's cache, so the next [token] call brings a fresh one. */
    fun clearToken(context: Context, token: String, done: () -> Unit) {
        val request = ClearTokenRequest.builder().setToken(token).build()
        Identity.getAuthorizationClient(context).clearToken(request)
            .addOnCompleteListener { task ->
                task.exception?.let { Log.w(TAG, "Clearing the token failed", it) }
                done()
            }
    }

    private fun request(): AuthorizationRequest =
        AuthorizationRequest.builder().setRequestedScopes(listOf(Scope(SCOPE))).build()

    private fun resultOf(result: AuthorizationResult): TokenResult {
        val intent = result.pendingIntent
        val token = result.accessToken
        return when {
            result.hasResolution() && intent != null -> TokenResult.NeedsUser(intent)
            token != null -> TokenResult.Token(token)
            else -> TokenResult.Failed(IllegalStateException("Google gave no access token"))
        }
    }

    private fun failureOf(error: Exception): TokenResult {
        Log.w(TAG, "Authorization failed", error)
        return if (error is ApiException && error.statusCode == CommonStatusCodes.DEVELOPER_ERROR) {
            TokenResult.NotRegistered
        } else {
            TokenResult.Failed(error)
        }
    }
}
