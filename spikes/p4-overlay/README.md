# p4-overlay spike: dark screen and white I-beam in Docs/Sheets (owner MSI test, 2026-11-06)

Throwaway measurement tools; nothing here ships.

- `dxgi_caps.py` (`python -I dxgi_caps.py`): per output `IDXGIOutput2::SupportsOverlays` and
  `IDXGIOutput6::CheckHardwareCompositionSupport`. Output on the test PC: `dxgi_caps.out.txt`.
- `pwcap.py`: PrintWindow(PW_RENDERFULLCONTENT) capture of one window (works on a locked PC); used for the p4-flicker numbers.
- `dx12-probe/`: times the DX12 adapter probe that `apps/*/src/graphics.rs::probe_dx12` runs at start.
- `pointer_sweep.rs`: copy into `upstream/wordcraft/crates/ui-egui/tests/` (do not commit it there) and run
  `cargo test -p wordcraft-ui-egui --test pointer_sweep -- --nocapture`: the cursor icon WordApp asks egui for down three
  vertical lines of a 1440x900 window. Output: `pointer_sweep.out.txt`.
