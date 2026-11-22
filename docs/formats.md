# Formats

What is known about the data, and how Vice Lens reads it. Offsets are little
endian unless stated. Facts here were checked on the 1.0.2.1 files.

## RPF9 archives (`9FPR`)

The directory (TOC) is encrypted, so there are no file paths. Payloads are
plain: zlib streams (`78 da`) at 16-byte alignment, larger ones starting at a
4 KiB boundary + 0x10 after zero padding. Vice Lens finds resources by scanning
for streams and recognising what they decompress to. Most resources carry their
own name inside (fwui prefabs and screens, drawables, texture dictionaries).

Files are stored in 64 KiB pieces: each piece is its own zlib stream, the next
piece starts at the next 16-byte boundary after the previous one ends, and a
piece shorter than 64 KiB ends the file. `data.rpf` scans in 2.5 s into 28 509
streams and 26 488 files.

RSC9 resources (pointers `0x5…` virtual, `0x6…` physical) start with 8 zero
bytes and a block map pointer; the map's bytes +8 and +9 count the virtual and
physical blocks that follow. A block is one stream, except that a block over
32 MiB is split into 32 MiB streams and a last, shorter one: only that one
ends the block. Counting streams instead cut the end off the two largest UI
texture dictionaries (their last textures pointed past the data read).

## Hashes

`joaat(lowercase(s))`, Jenkins one-at-a-time, as in earlier RAGE games. Text
labels use it (see below).

## RBF0 (binary XML, fwui UI data)

`RBF0`, then 4 bytes `fb ff 01 00`, then a token stream. Each token starts with
`u8 descriptor, u8 type`:

- `ff ff`: close the current element.
- `fd ff`: text content: `u32 length`, bytes (NUL-terminated UTF-8).
- otherwise the descriptor is an index into a name table; the first time an
  index appears it is followed by `u16 length` and the ASCII name.
  - type `0x00`: open element; `i16, i16, i16 attribute_count`, then that many
    `u8 descriptor, u8 type, value` attributes.
  - `0x10`: i64 value; `0x20`: true; `0x30`: false; `0x40`: f64 value;
    `0x60`: `u16 length` + UTF-8 string. These are leaf elements and do not
    take a close token.

data.rpf holds 1262 of them. Roots seen: `rage__fwuiPrefab` (603),
`rage__fwuiAnimationNodeGraph` (190), `UIScreen` (136), `CMovieSubtitleContainer`,
`UIMovieView` (65), `rage__fwuiFlowBlock` (61), `uiFeedbackContext` (28), ...

The menu: flow block `pause_main` loads `pause_menu/screens/pause_app_host` and
`pause_menu/screens/root/pause_menu_main` (the `UIScreen` named `SCRN_PauseMenu`,
inside the `UIMovieView` `MV_PauseMenu_Root`). Flow states are types such as
`rage__StateResourceLoader`, `StateUIObjectStreamedSceneHost`,
`PauseModeDecisionState`; their behaviour was engine code.

## Text tables (`0DRPz`)

A resource with `0DRPz` at 0x10. The large one in data.rpf (one per language)
is a Swiss-style hash table followed by an index array and string pages:

- Hash table at 0x60: 4096 groups of 16 control bytes + 16 slots of 16 bytes.
  Control `0x80` is an empty slot. A slot is `u32 hash, u32 0, u64 v`;
  `v >> 24` is the string index. `hash = joaat(lowercase(label))`.
- Index array (here at 0x310060): 16 bytes per string; `u64` at +8 holds the
  page in its low 16 bits and the byte offset in the page in `>> 20`.
- Pages: page 4 at 0x110060, 5 at 0x210060, 6 at 0x3a01e0 in the English table
  (`data.rpf` stream at 0x19be010). Vice Lens reads the block list from the
  header instead of these constants.

`PM_INDX_GAME` → "Game", `PM_INDX_MAP` → "Map", `PM_INDX_SET` → "Settings".

data.rpf holds 7 742 text tables in all. Besides the interface (8 languages,
36 888 labels each) are thousands of small ones (300 to 1 300 labels):
lines spoken in scenes, phone calls, by police and passers-by, posts and
messages. The translations of one table share its labels, so grouping
tables by their label sets and taking the most English-reading of each group
finds 586 English tables (246 055 lines). Lines carry markup: `~n~` a line
break, `~sl:start:length~` subtitle timing, `~b~` and other one-letter codes
for colour and weight, `~BLIP_…~` and `~INPUT_…~` icons.

## GFX (Scaleform)

SWF-like files with `GFX` signature. Six are font libraries with `DefineFont3`
(tag 75) vector glyphs: `futura_book`, `gta_art_deco_*`, `pricedown`,
`robin_pro_regular`, `machine_pro`, `noto_sans_sc_*`, `suite_*`, ...
One holds `platform_xbsx.input_icons` button glyphs.

## UI textures

fwui references `TextureDictionary` + `Texture` by name (`pausemenu_textures` /
`index_map`). The UI dictionaries are in `textures.rpf` (1356 dictionaries):
`pausemenu_textures` is the one at 0x916c010 (46 textures: `index_*`, `lock`,
`selection_arrow_*`), `controls_core` likely the one at 0x9493010
(`confirm_arrow`, `arrow`, `headset_off`, `lock`, ...). Dictionaries carry
texture names but not their own name, so Vice Lens matches a reference by the
texture name within dictionaries whose contents fit.

`pgDictionary<grcTexture>`: `+0x20` hashes, `+0x28` count, `+0x30` items,
`+0x38` count, `+0x3a` capacity. `grcTexture`: `+0x18` pixel pointer
(physical), `+0x30` name, `+0x38` width, `+0x3a` height, `+0x3f` DXGI format,
`+0x40` tiling (5 on every texture seen: AMD's `SW_4KB_S`, the 4 KiB
standard swizzle), `+0x42` mip count. Mips are stored smallest first, each
padded to whole tiles. Inside a tile the element index takes x and y bits
in this order, lowest first (measured: the order that makes each texture
smoothest, with the `texraw` and `swzsearch` examples):

| Bytes per element | Tile | Bits |
|---|---|---|
| 1 | 64×64 | x0 x1 x2 x3 y0 y1 y2 y3 y4 x4 y5 x5 |
| 2 | 64×32 | x0 x1 x2 y0 y1 y2 x3 y3 x4 y4 x5 |
| 4 | 32×32 | x0 x1 y0 y1 y2 x2 y3 x3 y4 x4 |
| 8 (BC1, BC4) | 32×16 | x0 y0 y1 x1 x2 y2 x3 y3 x4 |
| 16 (BC2, BC3, BC5–7) | 16×16 | y0 y1 x0 x1 y2 x2 y3 x3 |

Until the 1- and 4-byte rows were measured, A8, R8 and RGBA8 textures came
out scrambled.

TTPF packs (`RDS5` + `ETPF`): chunk table after the 24-byte header, texture
records (16 bytes) after `ETPF`, then a sorted table of 64-bit keys. A
record's third word holds the texture's start in 4 KiB pages in its low 21
bits: the 14 largest packs (`ttdb_maps_*`, `ttdb_terrain_*`) run past 4 GiB,
and read with 20 bits, the 21 601 textures there came from 4 GiB lower,
another texture's bytes: lime noise on roofs, rainbow noise on filing
cabinets and doors. The header's `+0x08` holds only the low 32 bits of the
stream's length; the chunk table gives all of it. Chunks are
raw, zlib, or BCPack (console-only). Larger mip-less textures use `SW_64KB_D`.
A mipmapped texture's top level ends its allocation, mips stored smallest
first; how far the allocation runs past the top level depends on size,
since every level takes whole 4 KiB tiles: a 128×256 BC1 texture's 16 KiB
top sits in 28 KiB, a 512×64 one's in 32 KiB (the smallest levels packed
tighter). A bound of 1.65 times the top level sent 10 915 such textures
(small or narrow ones: 128×256, 128×64, 64×64, 512×64) to the start of
their allocation, where their small mips are: footprints, shoe soles and
signs came out as noise. The whole chain, each level padded to tiles, is
the bound now.

## PSO (`PSIN`, with `LSCH`/`LENM`/`LSNS`)

Binary parsable-structure data. `PSIN` (big-endian chunk size): after the
8-byte chunk header, `u32`, `u32 region size`, then 16-byte block records
(`8 × 0xaa`, `u32 type hash`, `u32 size`) ended by a `0x70707070` record.
Block 1 is the region right after the records; blocks 2.. follow, 16-byte
aligned, in record order, and end exactly at the end of `PSIN`. A pointer is a
`u64`: block number in bits 0..12, byte offset in the block from bit 20.
Arrays are `ptr, u16 count, u16 capacity`. Names are joaat; `LSNS` looks
encrypted and is not needed for what Vice Lens reads.

### The UI style store (data.rpf 0x1c33f000 and 0x1c330000)

- Style map: block type `0xebbc845f`, 24-byte entries `u32 name hash, u32 0,
  ptr, u16 count` → array of pointers to states.
- State `0x936422a6`: `+8` array of property pointers, `+28` id. Id 1 is the
  normal text size; 3 and 4 hold larger text (52 → 68 → 84 for menu text).
- Text `0x817f7129` (48 bytes): `+8` joaat of a `$fontN` alias, `+0x10` f32
  tracking (thousandths of an em), `+0x1e` u16 size in 2160p pixels, `+0x23`
  1 = upper case.
- Brush `0x6909ec99`: three slots at `+8/+24/+40`; each points to a colour
  reference in block 2 (`u32 colour name hash, u32 alpha %`) or to a gradient
  `0xcb35f060` (`+16` stops: `f32 position, u32 colour hash, u32 alpha %`;
  `+0x48` angle in degrees; `+0x20` an unidentified 0.45/0.25). Vice Lens uses
  slot 0.
- Layout styles carry other property types (`0x8eb3d397` layout parameters,
  `0x0d061743` property-path overrides); not read yet.
- Conversions (small file): block type `0x100` maps a conversion name
  (`common_controls+list_item_text_body_conversion`) to a `0x90130782` object
  whose four style hashes at `+16` are focused-disabled, focused-enabled,
  disabled, enabled.

Not in the data: the RGB of named colours (`COLOR_TEXT_LIGHT_EMPHASIS_HIGH`,
...; the game asked its engine colour store) and which face each `$fontN`
alias means. Vice Lens's tables for both are in `crates/fwui/src/style.rs`.

## Movies (`movies.rpf`)

515 Bink 2 movies (`KB2n`), each starting on a 4 KiB boundary. Header: tag,
size - 8, frames, largest frame, -, width, height, fps, fps divisor, video
flags, audio tracks; one more word from revision `i`; per track the largest
decoded size; per track rate (u16) and flags (u16: 0x2000 stereo, 0x1000
DCT, 0x4000 16-bit); per track an id; then the frame offsets (bit 0 marks a
key frame). A frame opens with one packet per track (u32 size, then u32
decoded bytes and the bitstream), then the picture.

196 movies hold sound: 165 one track, 30 two, one four; all 48 kHz stereo
DCT. Ids are small numbers (0, 1, 2, 3); a movie's tracks are whole stereo
mixes of the same length (not surround channels), so other versions of the
sound rather than parts to mix. 16 tracks hold only silence. The audio is the
second version of Bink Audio (7-bit band quantisers, sign bits after the
coefficients); FFmpeg's Bink demuxer hands it to the decoder as the first,
which decodes it into full-scale noise. Vice Lens repacks each track's
packets as a `UEBA` stream (`crates/vice-lens/src/lens/bink.rs`), which
FFmpeg decodes as the second version.

The other 319 movies hold no track; the longest (112.7 s, 39.9 s) match no
stream length in `audio.rpf`, so any sound the game plays with them is
chosen by code.

65 movies (64 of the 67 phone-shaped 720×1280 clips, and a 1635×720 one)
fail in the Bink 2 video decoder of today's FFmpeg forks: too many frames do
not decode, and those that do are damaged. Their width leaves 16 pixels or
fewer of the last 32-pixel macroblock column. Per macroblock row the
bitstream codes edge flags for every 8-pixel column of whole macroblocks
(four per macroblock, less one), while the decoder counts them from the
width rounded up to 16 pixels, two columns short; every bit after is read
from the wrong place. Counting by whole macroblocks, and taking the stored
picture as whole macroblocks across for deblocking, decodes all 515 cleanly
(movie #110, 479 frames, checked frame by frame). The FFmpeg the releases
carry ([vice-lens-ffmpeg](https://github.com/Librett0/vice-lens-ffmpeg))
has this fix.

## The map

`common.rpf` manifests (`data/levels/americas/*.meta`) name the map's packs,
all nested inside `levels.rpf`: per 2048 m terrain group
(`map/terrain/n01_e01_n04_e04/`) a pack, `_metadata`, `_instance_placement`,
`_LOD`, `_terrain_images_merged`, `_bvh`; per area
(`map/environment/island_01/city/airport/ap_metadata.rpf`, `ap_merged.rpf`,
`ap_occl…`); archetypes in `.ityp` files. The nested TOCs are encrypted too,
so Vice Lens reads the payloads by kind.

Terrain: 64 m tiles named `n20_w05_037_hd` (blocks n01…n20 / s01…s17 north to
south, w20…e07 west to east, 512 m each, tiles 1…64), local coordinates
centred on the tile, heights included. `levels.rpf` holds 40 396 HD tiles.

### Drawables (`.#dr`, `.#ft`)

See `crates/rage/src/drawable.rs`. Local coordinates. Vertex streams are
typed: `0x42` 8 bytes (stream 0: packed position; later ones: two f32, UV),
`0x28` u16 UV (1/8192), `0x26` RGBA8 colour, `0x21` normals or tangents
(not read yet). The first UV stream is UV0, the next one UV1.

A shader struct is 0x40 bytes: `+0x00` the effect's hash (not the joaat of
the old effect names), then `meta`; `+0x10` texture references; `+0x20`
parameters; `+0x39` the render bucket in the low four bits, `+0x3c` the
same as a bit mask. Buckets, measured on walls, glass, decal sets,
foliage, fences and water, numbered as in the earlier RAGE games: 0
opaque, 1 alpha (glass, windows), 2 decal (dirt, markings, stains, drawn
over the surface below), 3 cut-out (leaves, fences), 6 water. Bit `0x80`
of the byte is a separate flag, set on most props. Water LOD tiles
(`*_water_slod*`, `*_waterveg_slod*`) are bucket 6 and have no albedo map.

Only a cut-out shader's albedo alpha is coverage. Opaque shaders keep other
data there: on the roof `dn_15b_apt1_top`, 13 opaque shaders have 17–43% of
their texels under 0.3, and cut by it, the roof opened onto the floors below
in white blotches. Railings are opaque meshes without such alpha. (The lime
squares on that roof were a texture read 4 GiB too low; see the TTPF notes.)

Albedo maps: shader parameters `diffusetex` (`0xc8e8c282`), `diffusesampler`
(`0xf1fe2b71`), `diffusetex2` (`0x0df47048`). Ground and terrain shaders
have none of these:

- 17 textures (terrain blend): `diffusetexture_layer0..3` (`0xce7b968e`,
  `0xd883aa9e`, `0x32b9df09`, `0x3d10f3b7`), `bumptexture_layer0..3`,
  `lookuptexture` (`0xc075f9b6`, red, green and blue streaks), `tinttexture`
  (`0x3fb7964f`, a grass-coloured map), and BC7/BC4 material maps. The
  layers tile by UV1 (some 20 repeats across a 64 m tile); UV0 spans the
  piece once. The lookup and tint maps are shared by many tiles (the same
  keys in `s03_w04_009_hd` and `s01_w01_058_hd`), so they are materials,
  not maps of one place.
- 12 textures (ground): three sRGB maps (`0x346394ea`, `0x7d4f12e2`,
  `0x7485dafb`; names unknown) with BC1/BC7 companions; often strips
  (2048×256) laid along kerbs and verges by UV0.

Skeletons: a drawable's `+0x18` points at its skeleton; there the bone count
is the u16 at `+0x6c` (`+0x68` holds a hash), the bone records (80 bytes:
local rotation and translation first, the name pointer at `+0x30`, next
sibling and parent as u16 at `+0x44` and `+0x46`) are at the pointer at
`+0x28`, and one 4×4 matrix per bone at `+0x30` (rows, translation last):
the inverse bind pose, taking the model's space into the bone's. Its
inverse places the bone: on a car, the wheels at their arches, the driver's
seat and steering wheel on the left (−x), the front at +y. Vehicle bodies
are not skinned piece by piece: doors, seats and panels are separate
pieces of surface in the body's meshes, found by joining triangles that
share corners and named by the bone each sits nearest.

Tuning parts are built in the vehicle's own space (spoilers, skirts, roofs,
diffusers, roof lights, tow hooks) or around one bone (bonnets around
`bonnet`, bumpers, splitters and grilles around `bumper_f`/`bumper_r`,
exhaust tips around `exhaust`/`exhaust_2`); each has a one-bone skeleton
named after itself, so the bone comes from the kind of part.

Vehicles: `<name>_hi` is the detailed body (some 170 000 triangles, with the
interior), `_mid` and the bare name the farther levels; tuning parts are
models of their own (`<name>_bonnet_02`, `<name>_spoiler_mu_04`, …), built
around their bone; wheels are rims (`rim_<style>_<nnn>`, a metre across and
half a metre wide) and tyres (`tyre_<kind>_<nnn>`, 1.58 m across around the
rim), scaled to the vehicle. The body's colour map is a key no pack holds:
the paint is made at runtime.

People are dressed from separate models in one bind pose:
`<item>_<body>_<nnn>_<details>_full` for clothes and accessories (bodies
`mr1`, `fr1`, `mb1`, `fb1`, `mo1`, `fo1`, `mk1`, `mc1`, `mi1`, `fc1`), with
`_cull_nn` versions that leave out what other clothes cover, first-person
`_fp` and torn `_aNN` ones; bare hands and feet (`hand_mr1_000_nude_cull`),
but no plain bare body: the `bodyuppergen`/`bodylowergen` models belong to
creatures (`sc_…`, `u_m_m_…`), and shop mannequins come in pieces (`mannequinupperbodyfigure_mr1_…`); a head as pieces worn
together (`head_<body>_<origin>_<nnn>_<age>`, `…_eyes`, `…_eyecap`,
`…_eyelashes`). Characters carry their own sets under a prefix: `cs_<name>_`
(cutscenes), `player_story000_<nn>_` (the protagonists' outfits), `hc_<group>_`
(crowd faces). Hair, brows and lashes hold a placeholder triangle in the
geometry Vice Lens reads. No strands are stored under their names: the
32-bit name hashes (`joaat` of the model name, with and without `_full`)
appear in no file or resource segment of `packs.rpf`, `anim.rpf`,
`data.rpf`, `common.rpf`, `story000.rpf`, `core_restofworld.rpf`,
`shared_minigame000.rpf`, `story000_restofworld.rpf`, `textures.rpf` or
`shaders.rpf` other than two tables, and the files beside each placeholder
are its own 256-byte vertex buffer. What a hairstyle has is a material record
in a PSO in `data.rpf` (0x1b894000, block type `0xaa690696`, 120 bytes each):
the model's name hash, four (texture key, flags) pairs and nine numbers
(two that look like colour, two lengths). The textures are BC7 maps in
`peds_loose.ttpf`: a 128×128 map of the scalp seen from above (white where
hair grows, the parting drawn in, the forehead at the top), a blank 32×32
one, and two tiling maps of strand directions. The strands are probably
grown from these at runtime. A 9.8 MB PSO in `packs.rpf` (0x11ccb2000) lists
the hairstyles among other items' tags.

Heads, creature bodies and some clothes (4 611 models) have a further file
after their vertex buffer, starting with a count and `0x80000000` (4 422
files, 89 MB in `packs.rpf`): perhaps shapes to blend (faces), not read yet.

Animals and other creatures are dressed the same way, one model per part:
`<class>_<species>_<sex>_<nn>_<part>`, the class `crm` mammals, `crb` birds,
`crr` reptiles, `crf` fish, `cra` amphibians, `crc` crustaceans, `cri`
insects, the sex `m`, `f` or `u`; parts `body_00`, `head_00`, `eyes_00`,
`teeth_00`, `fur_body_medium_00`, `feather_00`, … and named looks after the
number (`body_03_missmojo`, `costume_03_missmojo`, `head_12_discoaf`). A
few animals' own parts leave the number out (`crm_dog_carolina_m_body_00`,
`crm_cat_shorthairamerican_m_head_12_discoaf` of the animals `…_m_00`). The
bare animal name is a placeholder of its bounds that holds the skeleton.
Fur and feathers are placeholders like hair.

Every shader lists its textures by parameter name hash; known names include
`bumptex` (`0x1e77bf4c`, normals), `speculartex` (`0xc6f46e79`) and
`tintpalettetex` (`0x2c8d7182`). Props, buildings and terrain tiles name a
tint palette: the map that gives copies of one prop their different colours
(a bin's colour map is white). The palettes are in none of the TTPF packs: a
survey of every 60th model found 1 701 palette keys over 13 685 uses, and one
of them in a pack: a 4×4 placeholder. The keys appear nowhere else either
(searched as 64-bit values in every file and resource segment of `data.rpf`,
`common.rpf`, `textures.rpf`, `shaders.rpf` and `script.rpf`, and as their
32-bit halves in `data.rpf`'s PSO files), so the palettes are probably made at
runtime from data not found yet. Clothing has no colour map at all: a BC7 map of other
data, normals, a BC7 256×256 map and a BC4 mask, coloured by palettes and
masks the same way.

Vertex colour on terrain blend meshes is grey (R = G = B, 150 to 255) with
an alpha from 0 to 255: one value per vertex, probably what picks the layer,
the lookup map breaking up the edges. Not decoded yet, so Vice Lens draws
the first layer it can decode (often sand) tiled by UV1. Other shaders keep
other data there (R 255, G about 40, B and A varying).

Drawable dictionaries are pgDictionaries (`+0x20` name hashes, `+0x30`
drawables, u16 counts at `+0x28`/`+0x38`). `levels.rpf` holds 21 879 of
them (124 090 drawables): LOD stand-ins (15 826 sets, by the first entry's
name), super-LODs (2 686), water map tiles (506) and 2 861 other sets whose
entities are all LOD-level. A pass that reads each resource as one
drawable sees a dictionary as its first entry and hides the rest; the
first index pass did that, and with it 66 756 LOD entities had no model
(distant land drew as open sea).

Most models in the packs are dictionaries too, even files named `.#dr`:
89% of `packs.rpf`'s models (characters, animals, props), and about 10% of
`levels.rpf`'s. Each is a dictionary of one or a few drawables; the entry
whose name hash matches the model is the model (`drawable::parse_model`).

A packed position has two 24-bit slots and one 16-bit slot, each with its
own offset (`+0x60`) and scale (`+0x70`) in the vertex buffer header. The
slots are not always x, y, z: `+0x6c` names the axis in the 16-bit slot,
normally the mesh's thinnest, and the two 24-bit slots hold the others:

| `+0x6c` | slots hold | typical mesh |
|---|---|---|
| 0 | z, y, x | walls and signs facing ±x |
| 1 | z, x, y | signs and poles facing ±y |
| 2 | x, y, z | most buildings, roofs, ground |

Reading every mesh as x, y, z lays tall props and towers on their side and
mirrors the `0` case (its text reads backwards). The table was checked
against the declared bounds of ~350 meshes: every slot range matched the
axis it names, none negated. UVs follow Direct3D (v down): 82% of wall area
in a sample of buildings has v running downwards.

### Entities (`0DRPz` resources)

`0DRPz` is the console's parsable-data resource (text tables use it too):
header `+0x38` block list (`u32 type, u32 size, u64 pointer` into the virtual
or physical segment), `+0x4c` count; inner pointers use the PSO scheme
(block in bits 0..12, offset from bit 20). `levels.rpf` holds 188 449 of them.

- `0xce501483`: entities, 144 bytes: `+0x10` quaternion, `+0x38` world
  position, `+0x44` model (joaat of the drawable name), `+0x48` flags,
  `+0x54/+0x58` scale XY/Z, `+0x5c` parent (packed: `0xffff0000` on
  parentless LODs, -1 on parentless detailed entities), `+0x60` LOD
  distance (-1: the archetype's), `+0x64` the distance within which its
  children are drawn instead (101 m on a downtown LOD), `+0x68` the number
  of children (0 on detailed models). Flag `0x4` is set on stand-ins (and
  on some detailed models). Over the 2 039 513 map entities the LOD tree
  reads as a ladder of distances; each stand-in is drawn from its
  children's distance to its own:

  | level | children | children's distance | own LOD distance |
  |---|---|---|---|
  | detailed | 0 | | -1 (archetype) mostly; 50–200 or 1200 when set |
  | LOD | 1 | 171 m (median) | 450 m |
  | super-LOD 1 | 4 | 450 m | 1000 m |
  | super-LOD 2 | 16 | 1000 m | 2000 m |
  | super-LOD 3 | 4 | 2000 m | 4000 m |
  | super-LOD 4 | 1–4 | 4000–6000 m | 20 000 m |

  Vice Lens takes each stand-in's window from these two fields, filling an
  archetype's -1 by the next rung, and starts no stand-in inside the
  radius it draws detailed models in.
  Interiors (MLOs: shops, flats, offices, warehouses) keep their entities
  in their own coordinates around the origin, in the resource that defines
  them: 365 107 of the 2 039 513 map entities, in 7 135 resources (346 042
  within 16 m of the origin, larger interiors reaching 150 m). The origin is
  open water in the bay: drawn where they are, they piled up into a heap
  whose models did not fit in video memory. A definition is a block
  `0x10506455` (544 bytes): `+0x08` the interior's 64-bit hash, `+0x38` LOD
  distance, `+0x40`/`+0x4c` bounds, then arrays of a PSO pointer and a u16
  count: `+0xc0` its own entities (pointers to entity records: walls,
  shell, a few fixtures), `+0xd0` rooms, `+0xe0` portals, `+0xf0` entity
  sets. An entity set is 40 bytes: `+0x00` name hash, `+0x08` locations
  (rooms), `+0x18` entities. Scripts switch sets on; many are variants of
  one room's furnishing (39 for a house under construction, the same decals
  in each), and which are on by default is not in the data. Vice Lens takes
  them in order and leaves one out when over a third of its entities stand
  within 25 cm of what a set already taken put there: the first variant
  stays, sets that add something elsewhere are kept. Taking every set
  stacked up to 39 copies of a model in one place (2.1 million entities
  against 755 488). 6 296 other resources (39 622 entities) hold
  free-standing entity sets (block `0x35abc248`, 32 bytes, a name hash at
  `+0x0c`); Vice Lens leaves those out.

  The world places an interior with a block `0x09cc28be` (592 bytes) in an
  ordinary map resource: an entity record (rotation, position, a model
  hash that names no drawable, the interior's hash at `+0x20`) and fields
  of its own. Each of the interior's entities goes where the placement puts
  the interior's origin, turned by its rotation (`mapdata::place_in`).
  `levels.rpf` defines 1 706 interiors (348 of them with no entities: their
  contents are elsewhere, not found yet) and places them 2 590 times, at
  2 564 spots; every placement finds its interior there. The content packs
  and `data.rpf` define and place none. Seen in raw records: `+0x54` and
  `+0x58` are 1.0, `+0x5c` -1, `+0x60` -1.0, `+0x78` -1, `+0x80` 0xffff.
- `0x82d6fc83`: archetypes (each model's shared definition), 184 bytes:
  `+0x08` and `+0x20` a 64-bit hash, `+0x30` flags, `+0x38` LOD distance
  (a bin 60 m, an air conditioner 60, a traffic light 300, a 50 m ground
  tile 50), `+0x40`/`+0x4c` bounding box, `+0x58` sphere centre, `+0x64`
  radius, `+0x70` the distance within which full-size textures are wanted
  (11–33 m), `+0x74` the model name's joaat (again at `+0x78`, `+0x80`,
  `+0x88`). 583 715 records, 258 684 models. An entity's LOD distance of
  -1 means this one; without it, the contents of every shop downtown
  (shelves, jewellery, lamps) drew out to the detail radius.
- `0xba049770` + `0xea6b3a96`: instance batches, one model (its joaat in the
  header) and 64-byte records of a 3×4 world matrix plus scale and tint.
  About 1% of records read as matrices give absurd scales (1e12, or a Z
  column of 200 — likely a LOD distance): another record layout, dropped
  for now. Batches carry no LOD fields, and every rung of a prop is its
  own batch at the same spots: `prop_am_sign_trespass_01a` (archetype LOD
  distance 60 m), `prop_am_sign_trespass_01a_batchlod` (250 m), and shared
  stand-ins such as `prop_box_regiontint_batch_slod` (723 m) or
  `prop_cylinder_regiontint_batch_slod` (440 m). Each rung starts where the
  one below ends; started at 0 m, all three draw over each other. Plants
  work the same way with `_imposter` (a few cards): `veg_tree_palm_areca_sap_01`
  ends at 75 m and its imposter runs to 300 m; drawn together, a frame
  downtown held 6 614 imposters over their own trees.
- `0x7c475b5d` + `0xed86f522`: vegetation instancing (packed).
- `0x8056d01a`: lights.

Door data is PSO: 64-byte records with a u64 id, the model's joaat and a
world position.

### Navigation meshes (`navmeshes.rpf`)

See `crates/rage/src/navmesh.rs`. 128 m tiles in world coordinates; polygon
flags give road, pavement, water, interior.

### BCPack

Part of the TTPF texture data is stored in the console's BCPack codec
(chunk codecs 1–7), which Vice Lens does not decode. Codecs are chosen per
chunk, so a texture can mix them; of the 357 115 textures:

| Stored | Textures | Data |
|---|---|---|
| all raw or zlib | 306 320 (85.8%) | 113.1 GiB |
| only the small mips (stored first) in BCPack | 9 570 (2.7%) | 4.0 GiB |
| the top level in BCPack | 41 225 (11.5%) | 15.1 GiB |

The second kind decodes: its top level is zlib. Many of those are the
512×512 atlases of the super-LOD terrain (`s01_w01_s04_w04_slod4`, each
several square kilometres), which drew grey while any chunk of a texture
in BCPack ruled it out. The third kind draws grey. For 12 805 of them a
lower mip is in plain chunks, but lower mips are not read reliably yet:
against the top level shrunk, mip 1 usually matches, while levels under
128 texels often sit elsewhere (a packed mip tail), and some textures
match at no level (`crates/rage/examples/mipcheck.rs`).

## Skinning

A vertex buffer's stream slots 6 and 7 hold bone weights (type `0x26`, four
bytes each; the eight sum to 255), slots 8 and 9 the bones they belong to,
by type: `0x25` four u8, `0x4a` four u16, `0x8c` four u32 of which the low
16 bits count (heads), `0x00` one u8 and no weights (a vehicle's rigid
pieces). The bones index the model's palette (`+0x28`, its size at `+0x14`):
for characters and animals the sorted `joaat` of bone names (lower case),
matched against a skeleton held in another model: an animal's in the model
named as the animal (`crm_dog_chihuahua_f_00`, 142 bones), people's in the
crowd bodies (`ambient_malebody` 560 bones, `ambient_femalebody` 596,
`ambient_malebb`, `ambient_femalebb`, `ambient_kidbody`,
`ambient_infantbody`). On vehicles the palette is a bit set and the indices
are the vehicle's own bones: each door, bonnet, wheel hub and wiper follows
its bone, which stands on its hinge. Clothes add cloth bones (`GM_U01_…`) in
further skeletons 0x80 bytes after the first; a shirt's palette of 115 is 26
body bones and 89 of its own.

The bones a body's skeleton lacks are placed by Vice Lens, not by the data:

- A bone in the part's own skeleton with a parent chain rides the first
  ancestor the body has. A head's facial rig (`FACIAL_C_Jaw` under
  `FACIAL_C_FacialRoot`, which has no parent) rides `SKEL_Head`: animals'
  skeletons have no face bones, people's crowd bodies have the `FB_` ones.
  A part with no skeleton of its own (a dog's teeth) names its bones only
  by hash; the head beside it names them.
- Cloth bones have no parents, but their names give the side and the body
  part they hang on (`GM_L01_L_Calf02_CB`, `GM_U01_C_Spine5_RB`,
  `MH_Necklace_R_Delt_F`), and ride that part's bone (`SKEL_L_Calf`,
  `SKEL_Spine5`, `BM_R_Deltoid`). A vertex then keeps the blend of its
  cloth bones' parts, so a trouser leg bends at the knee. Placing them by
  the nearest bone instead tears a trouser leg by 17 cm in a run (a point
  low on the thigh lies nearer the knee's place than the hip's).
- Other bones (a watch's strap, a hat's band) ride the main bone nearest
  the vertex, measured to the bone's length.
- Vertices at one place, the two sides of a seam, take their bones
  together. A jacket's armhole has one side on `SKEL_L_UpperArm` and the
  other on a spine cloth bone; in a run it still opens by up to 14 cm,
  which the game's own rig for clothes evidently closes.

## Animations (`anim.rpf`)

`anim.rpf` (4.2 GB, 115 118 files) holds clip dictionaries (resources
without a type signature, e.g. `0x10023a010`, 11.7 MB) among 22 823 `0DRPz`
resources of scene data (`0xb45010`: events with start and end times; their
biggest blocks, `0x104` and `0x102`, are zero, space the game fills at
runtime). The dictionaries follow the earlier games' `crClipDictionary`
with every structure 0x10 bytes longer at its start:

- Dictionary: `+0x38` name, `+0x40` u16 bucket count (and a second count,
  not the clips'), `+0x48` buckets: chains of (u64 key, clip, next).
- Clip: `+0x20` type (2: a list of animations, the only one met), `+0x28`
  name (`pack:/gameplay@locomotion@male_generic@…/r-45_fwd_far_right.clip`),
  `+0x68` its animations (pointer, u16 count; entries of 0x38 bytes: the
  animation, its hash, `+0x28` start and `+0x2c` end in seconds, `+0x30`
  rate), `+0x78` length. A clip plays a stretch of each animation, and
  animations are packed: 24.8 s of `zzz_ml_biped_…_68.anim` hold several
  clips. A person's clip plays three: the body and face (116–163 tracks),
  the neck, head and toes (6) and the fingers (38).
- Animation (0x80 bytes): `+0x24` frames, `+0x26` frames a block starts
  apart (blocks overlap by one), `+0x28` length (s), `+0x30` name, `+0x50`
  blocks (pointers, u16 count), `+0x60` tracks (12 bytes each: the bone's
  name hash, the kind of track, its format: 0 a quaternion, 2 a vector, 4 a
  float). Kind `0x98e1750c` turns a bone and `0xf791a95f` places it, both
  relative to its parent and whole (not added to the skeleton's own pose: a
  clip's first frame matches the bind pose to a dot product of 0.99); the
  root is the exception, turned relative to its bind pose (the skeletons
  face the other way). Floats of kind `0x2deefadd` drive the face.
- Block: the earlier games' sequence: hash, length, 0, frame data offset,
  root motion offset, u16 0, frames, bytes per frame, indirect ints,
  quantised bits per frame, a byte (11 everywhere met), root motion counts;
  then the channels' parameters, the frames' packed bits (lowest bit first),
  the channel counts by type and per channel its track and component
  (`track * 4 + component`, each type's list padded to four with
  `tracks * 4`). Types 0–8 are the earlier ones (static quaternion, vector,
  float; raw, quantised and indirectly quantised floats; linear floats; a
  quaternion's last component computed from the other three) and read as
  they did (`rage::anim`). There are now 11: type 9 (not seen used) and
  type 10, which carries most of the motion where it is used (144 of 182
  channels in one block): incremental curves, coded as RDR2's type 9 but
  eight wide where that is four. A header like a linear float's (u32 words,
  the header included; u32 bit widths `c1 | c2 << 8 | c3 << 16`; f32
  quantum; f32 offset); then per chunk of 64 frames an offset (`c1` bits)
  and a start value (`c2` bits); then per group of eight frames of each
  chunk a width in unary (that many 0 bits, then a 1); then, from the next
  32-bit boundary, eight lanes interleaved word by word (word `w` of lane
  `l` is word `8w + l`). Frame `8g + 1 + l` of a chunk lies in lane `l`,
  as many bits as its group's width, each chunk starting at its offset in
  every lane. The values are second differences, zigzag coded (odd `r` is
  `-(r + 1) / 2`, even `r / 2`): each adds to a velocity that starts at 0
  in each chunk, the velocity adds to the value. A clip that keeps part of
  its motion in type 10 agrees with its LOD copy, which keeps none there,
  to 0.014° on average and 0.32° at worst (972 bone turns at the copy's
  keys).

Of 53 431 dictionaries, 45 423 hold clips: 273 759 of them, all of type 2
(a dictionary's map also holds entries with key 0 that are not clips).
Decoding and sampling every one (`crates/rage/examples/animcheck.rs`) finds
no channel of type 9, and all but 13 clips decode whole: those (in two
dictionaries, first-person pistol gestures and a plane's exit to swim) play
packed animations whose block pointer is null or points at data that is no
block. Every turn is a unit quaternion and every bone stays within 50 m of
its parent except, in some 150 clips, helper bones without vertices
(`PH_R_Hand`, `IK_L_Foot`, `WH_R_Hand`…) and props' bones, sent smoothly
tens of kilometres away, as when a prop is put in a pocket.

The other 8 008 dictionaries hold motions instead of clips: 16 629 entries
named `….motion` (and 655 others, such as the get-up sets of
`gameplay_ai_getup@male`, not read). A motion is clips of other
dictionaries the game plays together, picking and keeping them in step by
parameters each clip carries over its time
(`gameplay@locomotion@female_character@lucia@jog@run` holds
`jog_run_stop.motion`: the 26 clips of `…@lucia@jog@standing@run_stop`,
stopping from a jog towards each side on either foot):

- `+0x30` name; `+0x60` its clips (pointer, u16 count; 16 bytes each: a
  pointer filled at runtime, the `joaat` of the clip's name without
  `pack:/` and `.clip`, the `joaat` of its dictionary's name).
- `+0x70` per clip its length twice and a map of its time (pairs of
  times, the identity where met).
- `+0x80` per clip its parameters (pointer, u16 count; 32 bytes each: a
  pointer to keys, u16 count, then four floats), each key a value and a
  time in seconds. In `jog_run_stop` the first falls from 1.98 to 0 by
  1.3 s (how far is left to go) with floats (1.5, 0, 1.977, −1.977); the
  second rises by π a step and wraps (−π, π, 2π): the feet's phase.
  Combat motions (`run.motion`, `idle.motion`) carry none: the same clip
  in two or four dictionaries, one per weapon or hand.
- `+0xa0` and `+0xb0` two lists of two, the second naming the feet's tags
  (`leftfoot` `0x6d1bdcf4`, `rightfoot` `0x954b6425`).

The motions name 158 430 clips, 109 461 of them (69%) in dictionaries the
archives hold; the rest name dictionaries in none of them
(`…@dog@medium@generic@normal@trot@fast`, found by its name's hash). Vice
Lens lists a motion with its clips found in their dictionaries, each one
to play; the game's own blending of them is not done.

The content packs hold clip dictionaries too: `pack/story000/story000.rpf`
24 419 (140 669 clips: the story's scenes, recordings, safehouses and
19 143 cutscene shots), `story000_restofworld.rpf` 229 and
`core_restofworld001.rpf` 22; `packs.rpf`, `data.rpf` and `common.rpf`
none. A cutscene shot (`cin@clb_party_mcs1_office_scene_p1_pr1_t04`) has a
clip per actor, prop and camera (`lucia.clip`, `jason_face.clip`,
`prop_am_sofa_modern_01a.clip`, `rs_camera_10.clip`) playing 60-frame-a-
second animations (`cinematic_q8@female@0@60_1.anim`); a shot cut short
(`cin@ban_carte_ext_p1_t01-1`) holds the animations alone, without clips
(`cinematic_q8@crr_alligator_american_m_00@0@60_0.anim`: the actor's model
in the name), which Vice Lens plays whole. The people's face clips of the
shots met name a part with no animation (a null pointer). Decoding and
sampling the packs' 142 396 clips as `anim.rpf`'s finds one that does not
decode whole; besides helper bones, props and cameras sent far away (their
places in the scene), only a dog's `SKEL_ROOT` in 10 clips of one ship
scene lies more than 50 m from the origin.

A clip's face comes in a clip of its own beside it (`idle_a_face.clip`).
For an animal it drives the face's controls rather than its bones: floats
and pairs of kind `0x364d3bdb` on `CTRL_C_jaw`, `CTRL_L_nose`,
`CTRL_R_brow_down`… (the rig's `CTRL_analog_…` bones stay still). The game
turns controls into the facial rig's motion with expressions, and an
animal's mouth stays shut until they are read. They lie in `packs.rpf`
(16 resources of about 5 KB from `0x2f1010`, each naming `CTRL_C_jaw`
and `FACIAL_C_Jaw`): a hash at `+0x10`; at `+0x20` 52 inputs (8 bytes:
a control's hash and the kind `0x364d3bdb`); at `+0x60` 22 outputs (8
bytes: a bone's hash and the kind of a turn or a place; 11 bones, the
rig's `CTRL_analog_…`); index lists; and at `+0xa8` 561 entries from
`0x1c0`, among them `FACIAL_C_Jaw`'s hash: presumably the program that
computes the rig's motion from the controls, not decoded. The rig itself comes from the head
(`crm_dog_carolina_m_head_00`: 145 bones under `FACIAL_C_FacialRoot`,
placed in `SKEL_Head`'s frame; an eyeball lands within a centimetre of the
eye), which Vice Lens hangs under the animal's `SKEL_Head`, so the jaw,
tongue and teeth already follow their own bones.

`anim-dicts.tsv` (a cache step) lists the dictionaries of `anim.rpf` and
the content packs; `crates/rage/examples/animdump.rs` and `clipdump.rs`
show a dictionary's clips, tracks and values, `chantypes.rs` its channels
by type, `motiondump.rs` its motions; `drpztypes.rs` tallies the block
types of an archive.

A watch (`watchdress_fr1_…`) follows a `Watch_Case` bone and others its
palette names but the skeleton lacks; its vertices then land by the feet
once a clip plays, so it is placed in some other way not read yet.
