# Usage

### Running Rust Binaries

To execute the Rust binaries, use the following commands:

#### Generate Descriptions
```bash
RUST_LOG=info cargo run --bin descriptions --release /mnt/data/Photos/photos/
```

#### Generate Embeddings
```bash
RUST_LOG=info cargo run --bin embeddings --release /mnt/data/Photos/photos/
```

#### Dump Data
```bash
RUST_LOG=info cargo run --bin dump testdata/
```

---

# Ollama Installation

To install Ollama, run:

```bash
curl -fsSL https://ollama.com/install.sh
```

Alternatively, specify a version during installation:

```bash
curl -fsSL https://ollama.com/install.sh | OLLAMA_VERSION=0.4.0-rc5 sh
```

---

# Exiftool Commands

### Validating Files
```bash
exiftool -validate -warning -r /mnt/data/Photos/photos/2023/sizilien/
```

### Removing a Tag
```bash
exiftool -overwrite_original -IPTCDigest= -r /home/eric/Desktop/sizilien
```

### Updating a Tag
```bash
exiftool -overwrite_original -ExifVersion=0232 -r /home/eric/Desktop/sizilien
```

### Copying Tags
```bash
exiftool -all= -tagsfromfile @ -all:all -unsafe -overwrite_original -r /mnt/data/Photos/photos/2023/sizilien/
```

### Removing All XPComments
```bash
exiftool -overwrite_original -Exif:XPComment -r /mnt/data/Photos/photos/
```

### Cleanup Metadata
```bash
exiftool -overwrite_original -IFD0:ImageDescription= -Description= -xmp:description= -ExifIFD:MakerNotes= -iptc:Caption-Abstract= -ThumbnailImage= -r /mnt/data/Photos/photos/
```

### Dumping All XMP Information
```bash
exiftool -xmp -b -r /home/eric/Desktop/sizilien
```

### Additional Exiftool Commands

- Remove specific metadata:
  ```bash
  exiftool -exif:XPComment= -if '$XPComment' -r /mnt/data/Photos/photos/ -overwrite_original
  ```

- Identify images missing resolution data:
  ```bash
  exiftool -if 'not $exif:XResolution' -ext jpg -ext jpeg -r /mnt/data/Photos/photos/
  ```

- Standardize resolution and remove trailers:
  ```bash
  exiftool -trailer:all= -exif:XPComment= -exif:YResolution=72 -exif:XResolution=72 -exif:ResolutionUnit=inches -overwrite_original -ext jpg -ext jpeg -r /mnt/data/Photos/photos/
  ```

- Remove MPF metadata:
  ```bash
  exiftool -MPF:all= -if '$MPF:all' -r /mnt/data/Photos/photos/ -overwrite_original
  ```

---

# Tests

### Running Tests with Rust

#### Generate Descriptions
```bash
cargo run --bin descriptions ./testdata
```

---

# Performance Results

### llava:13b - Mac Pro M1
- **testdata/picasa/PXL_20230408_060152625.jpg:**
  > In a cozy, possibly European setting, a girl sits at a table with a white tablecloth, radiating joy as she smiles into the camera. The backdrop suggests it might be a traditional inn or restaurant.  
  **Time taken:** 16.91 seconds

- **testdata/sizilien/4L2A3805.jpg:**
  > The azure waters of Sicily welcome beachgoers to enjoy the tranquility under vibrant orange umbrellas, all nestled amongst the soft white sand.  
  **Time taken:** 12.77 seconds

### llava-phi3:latest - Mac Pro M1
- **testdata/picasa/PXL_20230408_060152625.jpg:**
  > A young girl in a purple sweater sits on a couch. The wall behind her is made of wood with a window on the left side, and there is a white curtain with floral patterns.  
  **Time taken:** 11.16 seconds

- **testdata/sizilien/4L2A3805.jpg:**
  > A large dog is sleeping on a beach in Sicily under umbrellas. The shore is surrounded by water and many chairs are on the beach for people to sit and enjoy the ocean views.  
  **Time taken:** 5.26 seconds

### llava:7b-v1.6-mistral-q5_1 - Mac Pro M1
- **testdata/picasa/PXL_20230408_060152625.jpg:**
  > A young girl is seated indoors, radiating joy with a wide smile on her face. She's dressed in casual attire and wearing a purple jacket with a blue zipper. The room around her seems cozy and comfortable, suggesting a warm, friendly environment. With a laptop in front of her and books scattered nearby, it appears she might be studying or working on a project.  
  **Time taken:** 17.62 seconds

- **testdata/sizilien/4L2A3805.jpg:**
  > This serene beach scene is characterized by several sun umbrellas set up on the pristine white sand. The tranquility is accentuated by a lone dog lounging nearby, its head resting lazily on the sandy shore, underlining the calm and quiet vibe of this coastal setting.  
  **Time taken:** 9.18 seconds

### llama-3.1-unhinged-vision-8b - RX 7600 XT
- **testdata/sizilien/4L2A3805.jpg:**
  > A serene beach scene unfolds before me, with the warm sand beneath my feet and the soothing sound of waves gently lapping at the shore. The vibrant hues of the umbrellas and lounge chairs stand out against the tranquil backdrop of the ocean, inviting relaxation and tranquility.  
  **Time taken:** 28.99 seconds  
  **Persons:** []

### llava:13b - RX 7600 XT
- **testdata/sizilien/4L2A3805.jpg:**
  > This sunny beach scene is characteristic of Sicilian coastline. The clear blue waters meet sand-colored shores under a backdrop of warm, bright skies. Tucked into the middle of this idyllic setting are rows of colorful umbrellas and lounge chairs, inviting beachgoers to relax and enjoy the seaside view. In the vicinity of these amenities, a dog is peacefully resting on the sand, adding to the tranquil atmosphere.  
  **Time taken:** 11.30 seconds  
  **Persons:** []

