# Privacy policy

This policy covers the Rnote builds published from this repository (the Android app
`io.github.yx14ng.rnote.viewer` and the desktop app `io.github.yx14ng.rnote`) and their
optional Google Drive sync.

## What the apps access

The apps keep your notebooks on your device. Only if you sign in with Google to turn on
the sync, they use Google Drive with the `drive.file` permission, which covers only the
files and folders the apps create themselves: a folder named `Rnote` in your Drive and
the notebooks and library file inside it. The apps can't see or change anything else in
your Drive.

## Where your data goes

Your notebooks travel only between your devices and your own Google Drive, directly. There
is no server of ours in between: nothing is sent to the developers or to anyone else, and
the apps have no analytics, ads or tracking of their own.

The sign-in that lets an app reach your Drive is kept only on the device that signed in.

## Handwriting search (Android)

Searching the handwriting uses Google's ML Kit Digital Ink Recognition, which reads the
handwriting on the device: the strokes are not sent anywhere. The first time a search
language is chosen, the app downloads that language's recognition model from Google's
servers. As with any app using ML Kit, ML Kit may also contact Google to get model
updates and sends Google metrics about how its recognition performs, without the
handwriting itself; see
<https://developers.google.com/ml-kit/android-data-disclosure>.

## Removing access

Signing out in the app stops the sync. You can also remove the apps' access at any time at
<https://myaccount.google.com/permissions>. The `Rnote` folder in your Drive stays yours:
deleting it is up to you.

## Google API services

The apps' use of information received from Google APIs adheres to the
[Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy),
including the Limited Use requirements.

## Contact

Questions about this policy: write to the support email shown on Google's sign-in screen
for Rnote.
