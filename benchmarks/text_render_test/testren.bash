#!/usr/bin/env bash

SCROLLBACK_TEST=false
for arg in "$@"; do
    if [ "$arg" = "--scroll-back" ]; then
        SCROLLBACK_TEST=true
    fi
done

clear

echo "==========================================="
echo "VELOX TERMINAL & FONT COMPATIBILITY BENCHMARK"
echo "==========================================="

sleep 1

#################################################
echo
echo "1. ASCII & BASE PRINTABLE CHARS"
echo

for i in {32..126}; do
    printf "\\$(printf '%03o' "$i") "
done

echo
sleep 0.5

#################################################
echo
echo "2. TEXT ATTRIBUTES & ANSI SGR STYLES"
echo

printf "Normal:          Hello Velox Terminal\n"
printf "\e[1mBold:\e[0m            Hello Velox Terminal\n"
printf "\e[2mDim:\e[0m             Hello Velox Terminal\n"
printf "\e[3mItalic:\e[0m          Hello Velox Terminal\n"
printf "\e[4mUnderline:\e[0m       Hello Velox Terminal\n"
printf "\e[21mDouble Under:\e[0m   Hello Velox Terminal\n"
printf "\e[4:3mCurly Under:\e[0m    Hello Velox Terminal\n"
printf "\e[5mBlink:\e[0m           Hello Velox Terminal\n"
printf "\e[7mReverse:\e[0m         Hello Velox Terminal\n"
printf "\e[8mHidden (Invisible):\e[0m [ \e[8mHIDDEN TEXT\e[0m ]\n"
printf "\e[9mStrikethrough:\e[0m   Hello Velox Terminal\n"
printf "\e[1;3;4;9mCombined (Bold+Italic+Underline+Strikethrough):\e[0m Hello Velox\n"

sleep 0.5

#################################################
echo
echo "3. DECSCUSR CURSOR SHAPES (BLOCK, UNDERLINE, BEAM)"
echo

printf "Testing Cursor Shape: Blinking Block (CSI 1 q)... "
printf "\e[1 q"
sleep 0.4
printf "\e[2 q"
printf "Steady Block (CSI 2 q)... "
sleep 0.4
printf "\e[3 q"
printf "Blinking Underline (CSI 3 q)... "
sleep 0.4
printf "\e[4 q"
printf "Steady Underline (CSI 4 q)... "
sleep 0.4
printf "\e[5 q"
printf "Blinking Beam (CSI 5 q)... "
sleep 0.4
printf "\e[6 q"
printf "Steady Beam (CSI 6 q)... "
sleep 0.4
printf "\e[0 q"
printf "Default Block restored.\n"

sleep 0.5

#################################################
echo
echo "4. BOX DRAWING & GRID ALIGNMENT"
echo

printf "┌───┬───┬───┐  ╔═══╦═══╦═══╗  ╭───┬───┬───╮\n"
printf "│ 1 │ 2 │ 3 │  ║ 1 ║ 2 ║ 3 ║  │ 1 │ 2 │ 3 │\n"
printf "├───┼───┼───┤  ╠═══╬═══╬═══╣  ├───┼───┼───┤\n"
printf "│ 4 │ 5 │ 6 │  ║ 4 ║ 5 ║ 6 ║  │ 4 │ 5 │ 6 │\n"
printf "├───┼───┼───┤  ╠═══╬═══╬═══╣  ├───┼───┼───┤\n"
printf "│ 7 │ 8 │ 9 │  ║ 7 ║ 8 ║ 9 ║  │ 7 │ 8 │ 9 │\n"
printf "└───┴───┴───┘  ╚═══╩═══╩═══╝  ╰───┴───┴───╯\n"

sleep 0.5

#################################################
echo
echo "5. BLOCK ELEMENTS & BRAILLE PATTERNS"
echo

echo "Blocks:  █ ▓ ▒ ░   ░ ▒ ▓ █"
echo "Ramp:    ▁ ▂ ▃ ▄ ▅ ▆ ▇ █"
echo "Braille: ⠁ ⠃ ⠇ ⠏ ⠟ ⠿ ⡿ ⣿   ⣿ ⡿ ⠿ ⠟ ⠏ ⠇ ⠃ ⠁"

sleep 0.5

#################################################
echo
echo "6. POWERLINE SYMBOLS & NERD FONTS"
echo

echo "Powerline Separators:               "
echo "Nerd Dev Icons:       󰣇 Arch  󰆍 Term  󰙯 Discord  󰈔 File  󰘚 Rust  󰊠 Git  󰊢 Commit  󰏗 Pkg  󰀵 Docker  󰌠 Python"

sleep 0.5

#################################################
echo
echo "7. OSC 8 HYPERLINKS & AUTO DETECTED URLS"
echo

printf "Explicit OSC 8 Link: \e]8;;https://github.com/lnoxsian/Velox\a[Velox GitHub Repository]\e]8;;\a\n"
echo "Auto-detected URL:   https://github.com/lnoxsian/Velox"

sleep 0.5

#################################################
echo
echo "8. EMOJIS, FLAGS & MIXED WIDTH CHARACTERS"
echo

echo "Emojis: 😀 😁 😂 🤣 😃 😄 😅 😆 😉 😊 😍 🥳 🤖 🚀 🎈 🎉 🦄 🦊"
echo "Flags:  🇺🇸 🇯🇵 🇮🇳 🇫🇷 🇬🇧 🇩🇪 🇨🇦 🇦🇺 🇧🇷 🇪🇸 🇮🇹 🇨🇳"
echo "Mixed Width: A中B文C国D語E (Double width CJK spacing check)"
echo "Mixed Emojis: Hello 🚀 World! 🤖 Hello 🇨🇳 CJK: 日本語"

sleep 0.5

#################################################
echo
echo "9. COMBINING CHARACTERS & INTERNATIONAL ALPHABETS"
echo

echo "Accents:   á é í ó ú"
echo "Zalgotext: Z͑̄͆ͭ̒̅"
echo "CJK:       日本語 漢字 한국어 中文"
echo "Cyrillic:  Привет, как дела? (Russian)"
echo "Greek:     Καλημέρα, τι κάνετε; (Greek)"
echo "Hindi:     नमस्ते दुनिया (Devanagari)"
echo "Arabic:    السلام عليكم (Arabic)"

sleep 0.5

#################################################
echo
echo "10. TRUECOLOR GRADIENTS & ANSI PALETTES"
echo
echo "256 Color Ramp:"

for i in {0..255}; do
    printf "\e[48;5;%sm " "$i"
done
printf "\e[0m\n\n"

echo "24-bit True Color RGB Gradient:"
for r in {0..255..16}; do
    for g in {0..255..16}; do
        printf "\e[48;2;%s;%s;64m " "$r" "$g"
    done
    printf "\e[0m\n"
done
printf "\e[0m\n"

sleep 0.5

#################################################
echo
echo "11. SGR COLON SUB-PARAMETERS & OSC 52 CLIPBOARD"
echo

printf "\e[4:1mUnderline Style 1 (Single) via Colons\e[0m\n"
printf "\e[4:2mUnderline Style 2 (Double) via Colons\e[0m\n"
printf "\e[4:3mUnderline Style 3 (Curly/Underline) via Colons\e[0m\n"
printf "\e[38:2::255:128:0mTruecolor Foreground (Orange) via Colons\e[0m\n"
printf "\e[48:2::0:128:255mTruecolor Background (Blue) via Colons\e[0m\n"

# Test OSC 52 Clipboard payload write
printf "\e]52;c;VmVsb3ggVGVybWluYWwgT1NDIDUyIENsaXBib2FyZCBUZXN0\a"
printf "OSC 52 Payload sent ('Velox Terminal OSC 52 Clipboard Test' copied to clipboard)\n"

sleep 0.5

#################################################
echo
echo "12. PROGRESS BARS & ANIMATED SPINNERS"
echo

spinner=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")
spinner_len=${#spinner[@]}

tput civis

for i in {0..100..4}; do
    filled=$((i / 4))
    empty=$((25 - filled))
    
    bar=""
    for ((k=0; k<filled; k++)); do bar="${bar}█"; done
    for ((k=0; k<empty; k++)); do bar="${bar}░"; done
    
    sp_frame=${spinner[$(( (i/4) % spinner_len ))]}
    
    r_val=$((255 - i * 2))
    g_val=$((i * 2))
    b_val=$((i * 255 / 100))
    
    printf "\r %s \e[1mLoading:\e[0m \e[38;2;%s;%s;%sm%s\e[0m %3d%% " "$sp_frame" "$r_val" "$g_val" "$b_val" "$bar" "$i"
    sleep 0.03
done

tput cnorm
echo
echo

if [ "$SCROLLBACK_TEST" = true ]; then
    echo "13. SCROLLBACK BUFFER STRESS TEST"
    echo "Generating 1,500 lines to stress test scrollback buffer..."
    sleep 0.5

    for i in {1..1500}; do
        echo "Scrollback line #$i - testing scrollback memory and limits"
    done

    echo "Scrollback buffer populated! You can scroll up to view the history."
    sleep 1
fi

echo
echo "==========================================="
echo "COMPATIBILITY & TEXT RENDERING BENCHMARK COMPLETE"
echo "==========================================="
echo
