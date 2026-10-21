# Rendering: inventario (2026-10-20, sola lettura)

**Stato 2026-10-20 sera:** i punti 2, 3 e 8.1 sono superati: `RetroMaterial` è diviso per superficie in `crates/game/src/materials/` (vedi il commento in `materials/mod.rs` e `docs/TODO.md`). Il resto vale ancora.

Fotografia di `crates/game/src` prima di una riorganizzazione. Niente è stato modificato per scriverla. **Cosa è verificato e cosa no** è marcato: «V» verificato leggendo il codice o provando, «S» stima da verificare.

Contesto: il 2026-10-20 il commit `ae08f8b` ha reso la build web inutilizzabile (panico wgpu GLES `index out of bounds: len 16, index 16`, il limite di 16 unità texture di WebGL2); corretto in `f57eb2d` con `cfg(wasm32)` sui campi `ground_*`. Nello stesso momento un'altra sessione sta spostando lo stato di bruciatura delle piante in texture (modifiche non committate in `retro.rs`, `vegetation.rs`, nuovi `veg_*.wgsl`; compare già `#[texture(112)]` in `RetroExtension`).

## 1. Dimensioni (V)

| File | Righe |
|---|---|
| `vegetation.rs` | 1906 |
| `fire_view.rs` | 1411 |
| `buildings.rs` | 915 |
| `units.rs` | 900 |
| `terrain_mesh.rs` | 776 |
| `shaders/retro.wgsl` | 741 |
| `sea.rs` | 454 · `town_kit.rs` 440 · `near_cover.rs` 387 · `sky.rs` 517 |

(`mario.rs` 1974 righe è a parte: easter egg con tre materiali propri.)

## 2. Famiglie di materiale (V)

Tre famiglie di tipo di materiale in Bevy:

**A. `ExtendedMaterial<StandardMaterial, _>`**: portano con sé tutto il layout PBR di Bevy (`StandardMaterial`) più i binding propri.

| Tipo | File | Binding propri | Texture proprie |
|---|---|---|---|
| `RetroMaterial` (`RetroExtension`) | `retro.rs` | 100 uniform, 101/102 strade, 103 uniform, 104/105 erba, 106 uniform, 107–110 colore+normale terreno (solo desktop), 111 uniform (solo desktop), 112 (nuovo, bruciatura, non committato) | 2 sul web, 5 su desktop, +1 con la bruciatura |
| `NearCoverMaterial` | `near_cover.rs` | 100 uniform, 101 (rilievo, R32Float), 102/103, 104/105 | 3 |

**B. `Material` propri** (layout leggero, nessun PBR): `FireMaterial`, `SmokeMaterial`, `FireGroundMaterial` (`fire_shader.rs`), `ZoneMaterial` (`forecast_zone.rs`), `FlatMaterial` (`flat.rs`), `ReachMaterial`, `RouteMaterial` (`reach_zone.rs`), `WaterMaterial` (`sea.rs`), `SkyMaterial` (`sky.rs`), `TapeMaterial` (`tape.rs`), `MarioMaterial`, `MarioOutlineMaterial`, `MarioMaskMaterial` (`mario.rs`). Ognuno ha 0–1 texture proprie. **Nessun problema di unità texture qui** (S: 1–3 unità di vista Bevy + la propria).

**C. Il resto**: egui (UI), `bevy_sprite`.

## 3. `RetroMaterial`: chi lo usa (V)

`material` / `material_with_style` / `finished_material` / `terrain_material`:

| Classe (`enabled.w`) | Chi | Costruttore |
|---|---|---|
| 1 terreno | `terrain_mesh.rs` | `terrain_material` |
| 2 vegetazione | `vegetation.rs` | `material_with_style` |
| 3 `Town` | `buildings.rs` | `finished_material` |
| 4 `Stone` | `landmarks.rs` | `finished_material` |
| 5 `Boat` | `boats.rs` | `finished_material` |
| 6 `Rock` | `coast.rs`, `landmarks.rs` | `finished_material` |
| 0 «generico» | `fields.rs`, `roads.rs`, `overlays.rs`, `units.rs` (4 punti), `kiosk/pins.rs` | `material` / `material_with_style` |

**Tutte le classi condividono lo stesso layout di binding** (`RetroExtension`) e lo stesso fragment shader (`retro.wgsl`, `fn fragment` con 13 punti che leggono `enabled.w`). Quindi:
- una texture aggiunta per il terreno si lega anche a edifici, barche, unità e pin che non la leggono mai (Bevy lega un segnaposto);
- il web ha subìto proprio questo.

## 4. Budget unità texture sul web (S, da misurare)

Limite WebGL2: 16 unità per stadio. Voci per un fragment di `RetroMaterial` (S: elenco da `bevy_pbr 0.14`, non contato sul codice):

- viste Bevy: ombre direzionali, ombre puntiformi, mappe ambiente (diffusa, speculare), eventuale trasmissione, prepass: circa 5–7;
- `StandardMaterial`: base colore, emissiva, metallic/roughness, occlusione, normal map, depth map, più i livelli opzionali (clearcoat, anisotropia): circa 6–9 (V sul layout, S sul numero in 0.14);
- `RetroExtension`: strade, erba (+ colore, normale sul desktop, + bruciatura oggi in corso).

L'errore è «len 16, index 16»: al web mancava **una** unità prima della correzione. Non esiste oggi un conteggio per materiale. **Il primo passo di analisi è misurarlo** (log o test sul layout compilato).

## 5. Dove sta la differenza web/desktop (V)

`cfg(target_arch = "wasm32")` / `cfg!(...)`: `retro.rs` 20, `quality.rs` 20, `playtest.rs` 10, `main.rs` 10, `kiosk/board.rs` 10, `mario.rs` 9, `terrain_mesh.rs` 7 (dopo `f57eb2d`), `kiosk/ui.rs` 7, `vegetation.rs` 6, `forecast_view.rs` 6, `kiosk/mod.rs` 5, `fire_view.rs` 4, `kiosk/screens.rs` 3, `sea.rs` 2, `ground_mask.rs` 1, `kiosk/view.rs` 1. Totale ~120 punti.

Profili grafici (`quality.rs`, V): `ALTA`, `MEDIA`, `BASSA` (desktop) e `WEB`, `WEB_MEDIA` (browser). Manopole: `dof`, `bloom`, `msaa`, `smaa`, `shadows`, `cascades`, `shadow_distance`, `shadow_map`, `vegetation`, `lod_m`, `terrain_shadows`, `terrain_stride`, `cover_m`. Nelle costanti per i profili ci sono altri `cfg!(wasm32)` dentro i valori (es. `vegetation`, `cover_m`). **Il web non è solo un profilo: è anche codice diverso** (campi, shader def, mesh per vertice).

## 6. Shader (V)

22 file WGSL, 4,1 k righe. Condivisi: `noise.wgsl`, `scene_light.wgsl` (luce + nebbia della scena: lo usano fuoco-suolo e nastro, non `retro`). `retro.wgsl` ha 741 righe, 13 funzioni per classe di superficie (terreno `terrain_albedo` 150 righe, vegetazione `foliage_albedo` + `burn_*`, `town_albedo`, `stone_albedo`, `rock_albedo`, `boat_albedo`) e importa la PBR di Bevy. **La luce di `retro` è quella PBR di Bevy; quella di fuoco-suolo/nastro è `scene_light`**: due strade diverse per ombre e nebbia.

Shader della bruciatura, in arrivo dall'altra sessione (non committati): `veg_state.wgsl`, `veg_vertex.wgsl`, `veg_prepass.wgsl`.

## 7. Post-processing e vista (V sui nomi, S sui costi)

Camera in `main.rs`: tonemapping, bloom (`BloomSettings`), profondità di campo (`DepthOfFieldSettings`), `Msaa` o SMAA, `CascadeShadowConfig`, nebbia (`fire_view.rs`, `fog`). Cambiati a caldo da `quality_live.rs` (inserisce/rimuove i componenti). Sul web: niente DoF/bloom/MSAA (`WEB`), ombre solo in `WEB_MEDIA`.

## 8. Cosa mi sembra il nodo (opinione, non verificata)

1. **Un layout, tante classi.** È la causa del panico di oggi e di ogni prossima texture. La soluzione naturale è un'estensione per classe (terreno / vegetazione / strutture / generico), con le funzioni di luce comuni importate.
2. **Il web è codice diverso, non un profilo.** I ~120 `cfg` e i `cfg!` dentro i profili rendono difficile sapere cosa gira sul browser senza provarlo.
3. **Due strade di illuminazione** (`retro` con PBR, il resto con `scene_light`): aspetto diverso e lavoro doppio per ombre e nebbia.
4. **Nessun controllo automatico sul web.** Il panico è passato in un commit senza che nessuno lo vedesse.
5. **Moduli molto grossi** (`vegetation.rs` 1900, `fire_view.rs` 1400 righe) che mescolano costruzione delle mesh, stato per frame e materiali.

## 9. Cosa non so e servirebbe sapere prima di decidere

- Quante unità texture usa ogni materiale sul web (misura).
- Quali campi di `StandardMaterial` si possono spegnere davvero in Bevy 0.14 e quante unità libererebbe un materiale del terreno personalizzato.
- Quanto costa in GPU ogni famiglia di materiale sul web (traccia delle prestazioni in partita: non ancora fatta, il clic nell'egui da CDP non funziona).
- Se i telefoni hanno le stesse 16 unità (WebGL2 garantisce 16 come minimo, ma i driver mobili non sono provati).
