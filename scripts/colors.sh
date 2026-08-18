#!/usr/bin/env bash
# Velox Terminal Color Palette Display

set -e

echo ""
echo -e "\033[1m  Velox Terminal Color Palette\033[0m"
echo -e "  ────────────────────────────"
echo ""

# 1. Standard and Bright ANSI Colors (0-15)
echo -e "\033[1;4mStandard ANSI Colors (0-7):\033[0m"
printf "  "
for i in {0..7}; do
    printf "\033[48;5;%sm    \033[0m " "$i"
done
echo ""
printf "  "
for i in {0..7}; do
    printf "\033[38;5;%sm %02d \033[0m " "$i" "$i"
done
echo ""
echo ""

echo -e "\033[1;4mBright / High-Intensity Colors (8-15):\033[0m"
printf "  "
for i in {8..15}; do
    printf "\033[48;5;%sm    \033[0m " "$i"
done
echo ""
printf "  "
for i in {8..15}; do
    printf "\033[38;5;%sm %02d \033[0m " "$i" "$i"
done
echo ""
echo ""

# 2. 256 Color Cube (16-231)
echo -e "\033[1;4m256-Color Cube (16-231):\033[0m"
for green in {0..5}; do
    printf "  "
    for red in {0..5}; do
        for blue in {0..5}; do
            color=$((16 + (red * 36) + (green * 6) + blue))
            printf "\033[48;5;%sm  \033[0m" "$color"
        done
        printf " "
    done
    echo ""
done
echo ""

# 3. Grayscale Ramp (232-255)
echo -e "\033[1;4mGrayscale Ramp (232-255):\033[0m"
printf "  "
for i in {232..255}; do
    printf "\033[48;5;%sm  \033[0m" "$i"
done
echo ""
echo ""

# 4. 24-bit Truecolor Smooth Gradient (RGB)
echo -e "\033[1;4m24-bit Truecolor Gradients:\033[0m"
printf "  "
for i in {0..79}; do
    r=$(( (i * 255) / 79 ))
    g=$(( 255 - (i * 255) / 79 ))
    b=$(( (i * 128) / 79 ))
    printf "\033[48;2;%s;%s;%sm \033[0m" "$r" "$g" "$b"
done
echo ""
printf "  "
for i in {0..79}; do
    # Rainbow spectrum
    hue=$(( (i * 360) / 79 ))
    h_idx=$(( hue / 60 ))
    f=$(( (hue % 60) * 255 / 60 ))
    q=$(( 255 - f ))
    case $h_idx in
        0) r=255; g=$f; b=0 ;;
        1) r=$q; g=255; b=0 ;;
        2) r=0; g=255; b=$f ;;
        3) r=0; g=$q; b=255 ;;
        4) r=$f; g=0; b=255 ;;
        *) r=255; g=0; b=$q ;;
    esac
    printf "\033[48;2;%s;%s;%sm \033[0m" "$r" "$g" "$b"
done
echo ""
echo ""
