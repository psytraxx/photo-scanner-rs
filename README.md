dump all xmp information

    exiftool -xmp -b -r /home/eric/Desktop/sizilien > test.txt 

copy all tags

    exiftool -tagsFromFile @ -all:all -icc_profile -overwrite_original -r /home/eric/Desktop/sizilien


cargo run  /home/eric/Desktop/sizilien/

others

//validate files
exiftool -validate -warning -r /home/eric/Desktop/sizilien

// remove a tag
exiftool -overwrite_original -IPTCDigest= -r /home/eric/Desktop/sizilien

//update a tag
exiftool -overwrite_original -ExifVersion=0232 -r /home/eric/Desktop/sizilien

exiftool -tagsFromFile @ -all:all -icc_profile -overwrite_original -r /mnt/data/Photos/photos/
cargo run --release /mnt/data/Photos/photos/2023/sizilien


exiftool -Exif:XPComment -r /mnt/data/Photos/photos/





672x672

llava:13b - mac pro m1
 INFO Description for testdata/picasa/PXL_20230408_060152625.jpg:  In a cozy, possibly European setting, a girl sits at table with a white tablecloth, radiating joy as she smiles into the camera. The backdrop suggests it might be a traditional inn or restaurant. Time taken: 16.91 seconds
 INFO Description for testdata/sizilien/4L2A3805.jpg:  The azure waters of Sicily welcome beachgoers to enjoy the tranquility under vibrant orange umbrellas, all nestled amongst the soft white sand. Time taken: 12.77 seconds


llava-phi3:latest - mac pro m1
INFO Description for testdata/picasa/PXL_20230408_060152625.jpg: A young girl in a purple sweater sits on a couch. The wall behind her is made of wood with a window on the left side, and there is a white curtain with floral patterns. Time taken: 11.16 seconds
 INFO Description for testdata/sizilien/4L2A3805.jpg: A large dog is sleeping on a beach in Sicily under umbrellas. The shore is surrounded by water and many chairs are on the beach for people to sit and enjoy the ocean views. Time taken: 5.26 seconds

llava:7b-v1.6-mistral-q5_1 - mac pro m1
 INFO Description for testdata/picasa/PXL_20230408_060152625.jpg:  A young girl is seated indoors, radiating joy with a wide smile on her face. She's dressed in casual attire and wearing a purple jacket with a blue zipper. The room around her seems cozy and comfortable, suggesting a warm, friendly environment. With a laptop in front of her and books scattered nearby, it appears she might be studying or working on a project.  Time taken: 17.62 seconds
 
 INFO Description for testdata/sizilien/4L2A3805.jpg:  This serene beach scene is characterized by several sun umbrellas set up on the pristine white sand. The tranquility is accentuated by a lone dog lounging nearby, its head resting lazily on the sandy shore, underlining the calm and quiet vibe of this coastal setting.  Time taken: 9.18 seconds
