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