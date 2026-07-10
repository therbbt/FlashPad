// The global hotkey is registered natively in Rust at startup (see
// GLOBAL_HOTKEY in src-tauri/src/main.rs) so it works instantly even while
// the window is hidden - routing it through the JS plugin API would require
// waking up a potentially-suspended webview for what should be an instant
// toggle. This class is a placeholder for when a settings UI exists to let
// a user change the hotkey at runtime, which would call a Rust command to
// re-register it rather than doing the toggle logic here.
export class HotkeyService {
  async register(_hotkey: string): Promise<void> {
    return;
  }
}
