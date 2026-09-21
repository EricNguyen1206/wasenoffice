import { installDesktopEditorBridge } from './shim/desktopEditor';

// Install the AscDesktopEditor bridge before anything engine-related loads.
installDesktopEditorBridge();

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <div>
    <h1>NanoOffice</h1>
    <p>Rust + Tauri shell around the ONLYOFFICE editing engine.</p>
    <p>PH-0 spike: load documenteditor + render a DOCX (next step).</p>
  </div>
`;

// Smoke check: bridge installed?
const bridge = window.AscDesktopEditor;
if (!bridge) {
  console.error('[nanooffice] bridge not installed');
} else {
  console.log('[nanooffice] bridge methods:', Object.keys(bridge).length);
}
