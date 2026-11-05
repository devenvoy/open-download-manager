# Browser installation

Browser packaging and native-host registration are paused during the Tauri
migration. The current checkout does not contain the previous Electron
packaging scripts. See commit `302fc1c` for the tracked pre-migration source;
do not use the historical workflow as evidence of Tauri compatibility.

The current preview automatically connects an extension after it is installed.
It does not yet install the extension in a new browser.

Browser Integration reports each browser separately. **Connected** means the app
found this extension in that browser's profile and successfully registered the
matching native messaging host. An installed browser without the extension is
shown as **Setup needed**; a missing executable is shown as **Not installed**.

For consumer Windows Chrome, external installation requires a published Chrome
Web Store extension. The desktop installer can register its store update URL;
Chrome then downloads the extension and asks the user to enable it. It cannot
silently enable a bundled unpacked extension. Edge supports external installation
from its own store or the Chrome Web Store.

Firefox 74 and newer does not allow an ordinary desktop application to silently
install its companion add-on. The supported Firefox option currently uses a temporary
flow; permanent distribution requires a signed XPI or an AMO listing.

## Prepare the submission

Resume this work after the Rust native-messaging host and Tauri packaging are
implemented and tested. Revalidate the extension package, permissions,
privacy disclosures, native messaging allowlist, and install/uninstall
registration against the migrated app before submitting it to a store.

Official distribution references:

- https://developer.chrome.com/docs/extensions/how-to/distribute/install-extensions
- https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/alternate-distribution-options
