# Character Creator

BlockGame features a built-in **Character Creator** with a stylized, articulated low-poly player model (chamfered head, distinct neck, shaped two-part torso, and elbow/knee-bent limbs) so your character feels uniquely yours across both singleplayer worlds and multiplayer servers.

## Accessing the Character Creator

The Character Creator is accessible from the **Main Menu**:

- Click **Character Creator** in the Main Menu left sidebar, **or**
- Click your **Character** portrait button inside the **Multiplayer & Servers** lobby.

*(To keep multiplayer sessions consistent, character editing is only available from the Main Menu—not from the in-game Pause Menu.)*

## Live 3D Turntable Preview

The left side of the Character Creator renders a live 3D preview of your character:

- **Drag** horizontally across the preview viewport to rotate your character 360°.
- **View: Face Zoom / Full Body** — Toggle between a close-up head view and a full-body view.
- **Pose: Idle / Walking** — Preview idle breathing or the articulated walking animation.
- **Randomize** — Generate a complete randomized character look in one click.

## Customization Options

All hairstyles, facial hair, and base clothing work on every **Body Frame**, and all Body Frames share the exact same gameplay collision hitbox (`0.6 × 1.8 × 0.6` blocks).

### Body Tab
- **Body Frame**: Choose between **Masculine**, **Feminine**, and **Neutral** silhouettes.
- **Skin Tone**: Pick from 12 curated natural & fantasy swatches or dial in any custom **RGB** color using the `R`, `G`, `B` sliders.

### Face & Hair Tab
- **Eye Style** (4 styles): `Standard`, `Almond`, `Wide`, `Sharp`.
- **Eye Color**: 10 curated iris swatches + full RGB sliders.
- **Hairstyle** (10 sculpted 3D styles): `Short Crop`, `Spiky`, `Swept Bangs`, `Side Part`, `Bob`, `Long Flowing`, `Ponytail`, `Twin Buns`, `Curly`, and `Bald`.
- **Hair Color**: 12 curated natural & vibrant swatches + full RGB sliders.
- **Facial Hair** (5 styles): `None`, `Stubble`, `Mustache`, `Short Beard`, `Full Beard` (available on any Body Frame; automatically matches your hair color).

### Clothing Tab
Base clothing is **purely cosmetic**—it does not take up inventory slots or affect gameplay armor stats.

- **Shirt Style** (6 styles): `T-Shirt`, `Long Sleeve`, `Tunic`, `Hoodie`, `Jacket`, `Vest` — with separate **Shirt Main** and **Shirt Accent** RGB colors.
- **Pants Style** (5 styles): `Trousers`, `Shorts`, `Skirt`, `Cargo`, `Cuffed` — with custom RGB color.
- **Shoe Style** (4 styles): `Boots`, `Sneakers`, `Sandals`, `Loafers` — with custom RGB color.

## Presets & Saving (`characters.yaml`)

Your active **Character Appearance** and custom **Character Presets** are stored locally in **`characters.yaml`** (next to `settings.cfg`), so your character follows you automatically into every singleplayer world and multiplayer server you join.

- **Starter Presets**: 6 read-only built-in presets (*Wayfarer*, *Forest Ranger*, *Gold Miner*, *Stargazer*, *Grove Warden*, and *Artisan*) spanning all three Body Frames. Selecting and editing one modifies your active appearance without overwriting the built-in template.
- **Custom Presets**: Open the **Presets** tab, enter a name, and click **Save Preset** (or press `Enter`) to save up to 24 custom presets. Click `X` on any custom preset to delete it.
- **Revert Changes**: Discard any edits made during the current Character Creator session.
- **Done (Save & Back)** / `Esc`: Save your active appearance to `characters.yaml` and return to the menu.

## In-Game & Multiplayer Appearance

- **First-Person View**: Your bare hand and held-item arm automatically match your character's skin tone, shirt sleeve style (bare arm for Vest, short sleeve for T-Shirt, full sleeve + accent cuff for Long Sleeve / Hoodie / Jacket / Tunic), and shirt colors.
- **Third-Person (`F5`) & Multiplayer**: Your full 3D stylized rig renders in third-person and is automatically synced to multiplayer servers during login so other players see your customized character and procedural 2D **Character Portrait** in the online player list (`Tab`).
