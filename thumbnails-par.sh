#!/bin/bash

# Input folder containing images
start_folder="$1"

# Check if the input folder is provided and exists
if [ -z "$start_folder" ] || [ ! -d "$start_folder" ]; then
    echo "Usage: $0 <folder_path>"
    exit 1
fi

# Function to process each image
process_image() {
    local input_image="$1"
    local thumbnail=$(mktemp /tmp/thumbnail.XXXXXX.jpg)

    echo "Processing $input_image"

    # Check if the image has an embedded thumbnail using exiftool
    thumbnail_present=$(exiftool -s -fast2 -ThumbnailImage "$input_image")
    
    if [ -z "$thumbnail_present" ]; then
        echo "Thumbnail missing in $input_image, embedding a new one."

        # Create a thumbnail with ImageMagick
        # Resize to 160x120 pixels (typical EXIF thumbnail size)
        convert "$input_image" -adaptive-resize 160x120 -strip -quality 75 "$thumbnail"

        # Embed the generated thumbnail back into the EXIF data of the original image
        exiftool -overwrite_original  \
                "-IFD1:ImageWidth=" \
                "-IFD1:ImageHeight=" \
                "-IFD0:ImageWidth=" \
                "-IFD0:ImageHeight=" \
                "-ExifImageWidth#=$(identify -format '%w' "$input_image")" \
                "-ExifImageHeight#=$(identify -format '%h' "$input_image")" \
                "-thumbnailimage<=$thumbnail" "$input_image"
    else
        echo "Thumbnail already present in $input_image, skipping update."
    fi

    # Clean up the temporary thumbnail file
    rm "$thumbnail"
}

export -f process_image

# Use find to locate all .jpg and .jpeg files in the folder structure recursively and process them in parallel
find "$start_folder" -type f \( -iname "*.jpg" -o -iname "*.jpeg" \) | parallel process_image

echo "Thumbnails embedded for all images in $start_folder"