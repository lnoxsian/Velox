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

echo "--- Style A: Smooth RGB Truecolor Block Bar ---"
for i in {0..100..2}; do
    filled=$((i / 2))
    empty=$((50 - filled))
    
    bar=""
    for ((k=0; k<filled; k++)); do bar="${bar}█"; done
    for ((k=0; k<empty; k++)); do bar="${bar}░"; done
    
    sp_frame=${spinner[$(( (i/2) % spinner_len ))]}
    
    r_val=$((255 - i * 2))
    g_val=$((i * 2))
    b_val=$((i * 255 / 100))
    
    printf "\r %s \e[1m[Core Download]:\e[0m \e[38;2;%s;%s;%sm%s\e[0m %3d%% " "$sp_frame" "$r_val" "$g_val" "$b_val" "$bar" "$i"
    sleep 0.015
done
echo -e "\n"

echo "--- Style B: Rounded Powerline Pill Bar ---"
for i in {0..100..4}; do
    filled=$((i / 5))
    empty=$((20 - filled))
    
    bar_fill=""
    for ((k=0; k<filled; k++)); do bar_fill="${bar_fill}━"; done
    bar_empty=""
    for ((k=0; k<empty; k++)); do bar_empty="${bar_empty}─"; done
    
    printf "\r \e[38;2;102;217;239m\e[48;2;102;217;239;38;2;30;30;30m %3d%% \e[48;2;50;50;50;38;2;166;226;46m%s\e[38;2;100;100;100m%s\e[48;2;102;217;239;38;2;30;30;30m  Velox \e[0;38;2;102;217;239m\e[0m" "$i" "$bar_fill" "$bar_empty"
    sleep 0.02
done
echo -e "\n"

echo "--- Style C: Sub-Block Smooth Gradient Bar ---"
sub_blocks=(" " "▏" "▎" "▍" "▌" "▋" "▊" "▉" "█")
for i in {0..100..2}; do
    total_eighths=$((i * 30 / 100 * 8 / 1))
    full_blocks=$((total_eighths / 8))
    rem_eighths=$((total_eighths % 8))
    
    bar=""
    for ((k=0; k<full_blocks; k++)); do bar="${bar}█"; done
    if [ $full_blocks -lt 30 ]; then
        bar="${bar}${sub_blocks[$rem_eighths]}"
        empty_count=$((29 - full_blocks))
        for ((k=0; k<empty_count; k++)); do bar="${bar} "; done
    fi
    
    printf "\r \e[1;33m⚡ Building:\e[0m [\e[38;2;255;184;108m%s\e[0m] \e[1;36m%3d%%\e[0m" "$bar" "$i"
    sleep 0.015
done
echo -e "\n"

echo "--- Style D: Multi-Task Concurrent Download Bars ---"
printf " Task 1 (Kernel Assets):   [                            ]   0%%\n"
printf " Task 2 (Font Atlas):      [                            ]   0%%\n"
printf " Task 3 (Shader Pipeline): [                            ]   0%%"

for i in {0..100..5}; do
    p1=$i
    p2=$(( i * 8 / 10 ))
    if [ $p2 -gt 100 ]; then p2=100; fi
    p3=$(( i * 12 / 10 ))
    if [ $p3 -gt 100 ]; then p3=100; fi
    
    # Task 3
    f3=$((p3 * 28 / 100)); e3=$((28 - f3))
    b3=""; for ((k=0; k<f3; k++)); do b3="${b3}█"; done; for ((k=0; k<e3; k++)); do b3="${b3}░"; done
    printf "\r Task 3 (Shader Pipeline): [\e[38;2;255;121;198m%s\e[0m] %3d%%" "$b3" "$p3"
    
    # Task 2
    f2=$((p2 * 28 / 100)); e2=$((28 - f2))
    b2=""; for ((k=0; k<f2; k++)); do b2="${b2}█"; done; for ((k=0; k<e2; k++)); do b2="${b2}░"; done
    printf "\e[1A\r Task 2 (Font Atlas):      [\e[38;2;80;250;123m%s\e[0m] %3d%%" "$b2" "$p2"
    
    # Task 1
    f1=$((p1 * 28 / 100)); e1=$((28 - f1))
    b1=""; for ((k=0; k<f1; k++)); do b1="${b1}█"; done; for ((k=0; k<e1; k++)); do b1="${b1}░"; done
    printf "\e[1A\r Task 1 (Kernel Assets):   [\e[38;2;139;233;253m%s\e[0m] %3d%%" "$b1" "$p1"
    
    printf "\e[2B"
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
