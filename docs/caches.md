# Caches

Vice Lens reads the game's archives in place. What it would otherwise find by
scanning tens of gigabytes on every start, it scans once and keeps in
`~/.cache/vice-lens/`. Everything there is made from your own copy and stays on
your machine.

Vice Lens makes them from Setup (**Build missing caches**); on the command
line it is `vice-lens --build-caches --game /path/to/your/game/folder` (or
`tools/build-caches.sh /path/to/your/game/folder`). Steps run in order and
skip what is already there; delete a file to make it again. On a fast SSD
the whole run takes about an hour and leaves about 2 GB.

| Step | File | `rage::index` | What it holds |
|---|---|---|---|
| 1 | `survey-levels.tsv` | `survey` | every file in `levels.rpf`: offset, size, pieces, what it looks like |
| 2 | `models.tsv` | `drawables` | every drawable: `archive<TAB>offset<TAB>name` |
| 3 | `dicts-levels.tsv` | `dictionaries` | drawables kept in dictionaries (the LOD and super-LOD models): `offset<TAB>hash<TAB>entry<TAB>first name` |
| 4 | `levels-drpz.txt` | `map_data` | offsets of the map data resources (`0DRPz`; `TEXT` in the survey) |
| 5 | `entities.bin` | `entities` | every placement on the map, 60 bytes each |
| 6 | `instances.bin` | `entities` (instances) | batch instances (trees, grass, small props), 48 bytes each |
| 7 | `archetypes.bin` | `archetypes` | each model's LOD distance and size, 16 bytes each |
| 8 | `interiors.bin` | `interiors` | the entities of every interior (shops, flats, offices) where the world places it, 60 bytes each as in `entities.bin`; reads all of `levels.rpf`'s map data again (about 10 minutes) |
| 9 | `map.png`, `map-clay.png`, `map-relief.png`, `map.txt` | `map::make` (vice-lens) | the island seen from above, 2 m a pixel, in three styles (satellite, clay, relief), and where it lies; about 12 minutes on the GPU. A map made before the styles is made again |
| 10 | `models-packs.tsv` | `models` | the models of `packs.rpf`, `data.rpf` and the content packs under `pack/` (characters, animals, props): `archive<TAB>offset<TAB>name`, a dictionary once per entry |
| 11 | `anim-dicts.tsv` | `anim::survey` | every clip dictionary in `anim.rpf` (53 431) and the content packs (24 671): `offset<TAB>name<TAB>clips<TAB>1` when every clip decodes, else `0`, then the archive for a pack's (`pack/story000/story000.rpf`); one pass over each archive, about two minutes in all |

The `rage` examples (`survey`, `unnamed`, `dicts`, `entities`,
`archetypes`) run single steps by hand. [formats.md](formats.md) describes
what they read.

Vice Lens also writes `survey-<archive>.tsv` when you list an archive under
Files. `navmesh.tsv` (the navigation mesh tiles by name) is optional: the
world view only uses it for `--nav` and the ground height under the start. The
first text search writes `text-subtitles.txt`: where the English interface
table and the 586 English tables of dialogue and posts lie in `data.rpf`
(found once by reading all 7 742 tables, a few seconds), so later runs read
only those. Movie conversions, posters and stills go to `media/`.
