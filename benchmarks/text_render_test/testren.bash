#!/usr/bin/env bash

SCROLLBACK_ONLY=false
GRID_ONLY=false

for arg in "$@"; do
    if [ "$arg" = "--scroll-back" ] || [ "$arg" = "--scrollback" ]; then
        SCROLLBACK_ONLY=true
    fi
    if [ "$arg" = "--grid" ] || [ "$arg" = "--full-grid" ]; then
        GRID_ONLY=true
    fi
done

clear

if [ "$GRID_ONLY" = true ]; then
    echo "==========================================="
    echo "VELOX TERMINAL GRID LAYOUT TEST"
    echo "==========================================="
    echo

    cols=$(tput cols 2>/dev/null || echo 80)
    lines=$(tput lines 2>/dev/null || echo 24)

    echo "Detected Terminal Dimensions: ${cols} columns × ${lines} rows"
    echo "Rendering explicit column numbers (1..${cols}) and row numbers (1..${lines}) matrix grid..."
    echo

    # 1. Column Tens & Units Header Rulers
    tens_line="Col:  "
    units_line="      "
    for ((c=7; c<=cols; c++)); do
        unit=$((c % 10))
        if [ $((c % 10)) -eq 0 ]; then
            ten_digit=$(( (c / 10) % 10 ))
            tens_line="${tens_line}${ten_digit}"
        else
            tens_line="${tens_line} "
        fi
        units_line="${units_line}${unit}"
    done

    printf "\e[1;33m%s\e[0m\n" "$tens_line"
    printf "\e[1;36m%s\e[0m\n" "$units_line"

    # 2. Top Box Line
    grid_width=$((cols - 8))
    if [ $grid_width -lt 10 ]; then grid_width=10; fi

    border_h=""
    for ((k=0; k<grid_width; k++)); do border_h="${border_h}─"; done

    printf "\e[1;35m┌──────┬%s┐\e[0m\n" "$border_h"

    max_rows=$((lines - 10))
    if [ $max_rows -lt 4 ]; then max_rows=4; fi

    for ((r=1; r<=max_rows; r++)); do
        row_tag=$(printf "R%02d" "$r")
        cell_pattern=""
        for ((c=1; c<=grid_width; c++)); do
            if [ $((r % 2)) -eq 1 ]; then
                cell_pattern="${cell_pattern}$((c % 10))"
            else
                if [ $((c % 5)) -eq 0 ]; then
                    cell_pattern="${cell_pattern}+"
                else
                    cell_pattern="${cell_pattern}·"
                fi
            fi
        done
        printf "\e[1;35m│\e[1;32m %s \e[1;35m│\e[0m%s\e[1;35m│\e[0m\n" "$row_tag" "$cell_pattern"
    done

    printf "\e[1;35m└──────┴%s┘\e[0m\n" "$border_h"
    echo

    echo "Launching Full-Screen Alternate Buffer Corner-to-Corner Sweep..."
    sleep 0.5

    printf "\e[?1049h\e[?25l\e[2J\e[H"

    # Row 1: Column Header Ruler
    printf "\e[1;1H\e[1;44;37m"
    for ((c=1; c<=cols; c++)); do
        printf "%d" "$((c % 10))"
    done
    printf "\e[0m"

    # Middle Rows: Row Index + Matrix Grid Cells
    for ((r=2; r<lines; r++)); do
        printf "\e[%d;1H\e[1;33m%02d:\e[0m" "$r" "$r"
        grid_fill=""
        for ((c=4; c<=cols; c++)); do
            if [ $c -eq 4 ] || [ $c -eq $cols ]; then
                grid_fill="${grid_fill}│"
            elif [ $((c % 10)) -eq 0 ]; then
                grid_fill="${grid_fill}┼"
            elif [ $((r % 2)) -eq 0 ]; then
                grid_fill="${grid_fill}$((c % 10))"
            else
                grid_fill="${grid_fill}·"
            fi
        done
        printf "%s" "$grid_fill"
    done

    # Bottom Row: Corner Markers & Dimension Summary
    printf "\e[%d;1H\e[1;44;37m" "$lines"
    for ((c=1; c<=cols; c++)); do
        printf "%d" "$((c % 10))"
    done
    printf "\e[0m"

    # Overlay ESC Cancellation Hint & Corner Badges
    printf "\e[2;5H\e[1;101;37m ↖ (1,1) TOP-LEFT \e[0m"
    printf "\e[2;%dH\e[1;101;37m (1,%d) TOP-RIGHT ↗ \e[0m" "$((cols - 20))" "$cols"

    mid_r=$((lines / 2))
    mid_msg=" VELOX FULL MATRIX GRID [ %d ROWS x %d COLS ] " "$lines" "$cols"
    mid_msg_str=$(printf "$mid_msg")
    mid_c=$(( (cols - ${#mid_msg_str}) / 2 ))
    if [ $mid_c -lt 1 ]; then mid_c=1; fi
    printf "\e[%d;%dH\e[1;42;30m%s\e[0m" "$mid_r" "$mid_c" "$mid_msg_str"

    hint_r=$((lines - 2))
    hint_msg=" Press [ESC] or [q] to exit grid test "
    hint_c=$(( (cols - ${#hint_msg}) / 2 ))
    if [ $hint_c -lt 1 ]; then hint_c=1; fi
    printf "\e[%d;%dH\e[1;43;30m%s\e[0m" "$hint_r" "$hint_c" "$hint_msg"

    bot_r=$((lines - 1))
    printf "\e[%d;5H\e[1;101;37m ↙ (%d,1) BOTTOM-LEFT \e[0m" "$bot_r" "$lines"
    printf "\e[%d;%dH\e[1;40;37m (%d,%d) BOTTOM-RIGHT ↘ \e[0m" "$bot_r" "$((cols - 24))" "$lines" "$cols"

    read -t 5 -n 1 -r -s key
    if [[ "$key" == $'\e' ]] || [[ "$key" == "q" ]] || [[ "$key" == "Q" ]]; then
        :
    fi

    printf "\e[?1049l\e[?25h"
    echo
    echo "==========================================="
    echo "GRID TEST COMPLETE"
    echo "==========================================="
    echo
    exit 0
fi

if [ "$SCROLLBACK_ONLY" = true ]; then
    echo "==========================================="
    echo "VELOX SCROLLBACK BUFFER STRESS TEST"
    echo "==========================================="
    echo
    echo "Generating 1,500 lines to stress test scrollback buffer..."
    sleep 0.5

    for i in {1..1500}; do
        echo "Scrollback line #$i - testing scrollback memory and limits"
    done

    echo
    echo "Scrollback buffer populated! You can scroll up to view the history."
    echo
    echo "==========================================="
    echo "SCROLLBACK TEST COMPLETE"
    echo "==========================================="
    echo
    exit 0
fi

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

echo -e "\n"
echo "--- Style E: Package Installer Loading Bars (APT, Cargo, Pacman, Npm, Pip) ---"

# 1. APT Package Manager
printf "\e[1;34mGet:1\e[0m http://archive.ubuntu.com/ubuntu jammy/main amd64 velox-core 0.1.5 [1,248 kB]\n"
for i in {0..100..5}; do
    filled=$((i * 35 / 100))
    empty=$((35 - filled))
    bar_fill=""
    for ((k=0; k<filled; k++)); do bar_fill="${bar_fill}#"; done
    bar_empty=""
    for ((k=0; k<empty; k++)); do bar_empty="${bar_empty}."; done
    printf "\rReading database ... %3d%% [ \e[1;32m%s\e[0m\e[1;30m%s\e[0m ] (%d/35 packages)" "$i" "$bar_fill" "$bar_empty" "$((i * 35 / 100))"
    sleep 0.015
done
echo -e "\n"

# 2. Cargo (Rust) Package Build
pkgs=("serde_json v1.0.114" "tokio v1.36.0" "glam v0.27.0" "glow v0.16.0" "velox v0.1.5")
for p in "${!pkgs[@]}"; do
    pkg_name="${pkgs[$p]}"
    step=$(( (p + 1) * 20 ))
    filled=$(( (p + 1) * 6 ))
    empty=$(( 30 - filled ))
    bar_fill=""
    for ((k=0; k<filled; k++)); do bar_fill="${bar_fill}="; done
    bar_empty=""
    for ((k=0; k<empty; k++)); do bar_empty="${bar_empty} "; done
    printf "\r  \e[1;32mCompiling\e[0m %-20s [\e[1;36m%s>%s\e[0m] %d/5 (%d%%)" "$pkg_name" "$bar_fill" "$bar_empty" "$((p + 1))" "$step"
    sleep 0.04
done
echo -e "\n"

# 3. Arch Pacman Package Manager
for i in {0..100..4}; do
    filled=$((i * 30 / 100))
    empty=$((30 - filled))
    bar_fill=""
    for ((k=0; k<filled; k++)); do bar_fill="${bar_fill}#"; done
    bar_empty=""
    for ((k=0; k<empty; k++)); do bar_empty="${bar_empty}-"; done
    printf "\r\e[1;36m(1/1) upgrading velox-terminal       \e[0m[\e[1;33m%s\e[0m\e[1;30m%s\e[0m] %3d%%" "$bar_fill" "$bar_empty" "$i"
    sleep 0.015
done
echo -e "\n"

# 4. NPM / Node Package Manager
npm_spin=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")
for i in {0..100..5}; do
    filled=$((i * 25 / 100))
    empty=$((25 - filled))
    bar_fill=""
    for ((k=0; k<filled; k++)); do bar_fill="${bar_fill}█"; done
    bar_empty=""
    for ((k=0; k<empty; k++)); do bar_empty="${bar_empty}░"; done
    sp=${npm_spin[$(( (i/5) % 10 ))]}
    printf "\r \e[38;2;203;56;55m%s npm\e[0m \e[1;37mreify:velox:\e[0m \e[38;2;80;250;123mtree:\e[0m[\e[38;2;139;233;253m%s\e[38;2;90;90;90m%s\e[0m] \e[1mhttp fetch GET 200\e[0m %3d%%" "$sp" "$bar_fill" "$bar_empty" "$i"
    sleep 0.02
done
echo -e "\n"

# 5. Pip (Python) Package Downloader
for i in {0..100..5}; do
    filled=$((i * 30 / 100))
    empty=$((30 - filled))
    bar_fill=""
    for ((k=0; k<filled; k++)); do bar_fill="${bar_fill}━"; done
    bar_empty=""
    for ((k=0; k<empty; k++)); do bar_empty="${bar_empty}╸"; done
    mb=$((i * 142 / 100))
    printf "\rDownloading velox_gui-0.1.5-py3-none-any.whl (%d.2/14.2 MB)\n \e[38;2;255;121;198m━━━━━━━━━━━━━━━━━━━━\e[0m [\e[38;2;80;250;123m%s\e[38;2;90;90;90m%s\e[0m] %3d%% 2.4 MB/s eta 0:00:01\e[1A" "$mb" "$bar_fill" "$bar_empty" "$i"
    sleep 0.02
done
echo -e "\n"

tput cnorm
echo
echo

#################################################
echo "13. CORNER-TO-CORNER FULL GRID LAYOUT TEST"
echo

cols=$(tput cols 2>/dev/null || echo 80)
lines=$(tput lines 2>/dev/null || echo 24)

echo "Detected Terminal Dimensions: ${cols} columns × ${lines} rows"
echo "Rendering explicit column numbers (1..${cols}) and row numbers (1..${lines}) matrix grid..."
echo

# 1. Column Tens & Units Header Rulers
tens_line="Col:  "
units_line="      "
for ((c=7; c<=cols; c++)); do
    unit=$((c % 10))
    if [ $((c % 10)) -eq 0 ]; then
        ten_digit=$(( (c / 10) % 10 ))
        tens_line="${tens_line}${ten_digit}"
    else
        tens_line="${tens_line} "
    fi
    units_line="${units_line}${unit}"
done

printf "\e[1;33m%s\e[0m\n" "$tens_line"
printf "\e[1;36m%s\e[0m\n" "$units_line"

# 2. Top Box Line
grid_width=$((cols - 8))
if [ $grid_width -lt 10 ]; then grid_width=10; fi

border_h=""
for ((k=0; k<grid_width; k++)); do border_h="${border_h}─"; done

printf "\e[1;35m┌──────┬%s┐\e[0m\n" "$border_h"

# 3. Data Rows with Row Index & Column Unit Patterns
max_rows=8
if [ $((lines / 2)) -gt 8 ]; then max_rows=$((lines / 2)); fi

for ((r=1; r<=max_rows; r++)); do
    row_tag=$(printf "R%02d" "$r")
    cell_pattern=""
    for ((c=1; c<=grid_width; c++)); do
        if [ $((r % 2)) -eq 1 ]; then
            cell_pattern="${cell_pattern}$((c % 10))"
        else
            if [ $((c % 5)) -eq 0 ]; then
                cell_pattern="${cell_pattern}+"
            else
                cell_pattern="${cell_pattern}·"
            fi
        fi
    done
    printf "\e[1;35m│\e[1;32m %s \e[1;35m│\e[0m%s\e[1;35m│\e[0m\n" "$row_tag" "$cell_pattern"
done

printf "\e[1;35m└──────┴%s┘\e[0m\n" "$border_h"
echo

if [ "$FULL_GRID_TEST" = true ]; then
    echo "Launching Full-Screen Alternate Buffer Corner-to-Corner Sweep..."
    sleep 0.8

    printf "\e[?1049h\e[?25l\e[2J\e[H"

    # Row 1: Column Header Ruler
    printf "\e[1;1H\e[1;44;37m"
    for ((c=1; c<=cols; c++)); do
        printf "%d" "$((c % 10))"
    done
    printf "\e[0m"

    # Middle Rows: Row Index + Matrix Grid Cells
    for ((r=2; r<lines; r++)); do
        printf "\e[%d;1H\e[1;33m%02d:\e[0m" "$r" "$r"
        grid_fill=""
        for ((c=4; c<=cols; c++)); do
            if [ $c -eq 4 ] || [ $c -eq $cols ]; then
                grid_fill="${grid_fill}│"
            elif [ $((c % 10)) -eq 0 ]; then
                grid_fill="${grid_fill}┼"
            elif [ $((r % 2)) -eq 0 ]; then
                grid_fill="${grid_fill}$((c % 10))"
            else
                grid_fill="${grid_fill}·"
            fi
        done
        printf "%s" "$grid_fill"
    done

    # Bottom Row: Corner Markers & Dimension Summary
    printf "\e[%d;1H\e[1;44;37m" "$lines"
    for ((c=1; c<=cols; c++)); do
        printf "%d" "$((c % 10))"
    done
    printf "\e[0m"

    # Overlay ESC Cancellation Hint & Corner Badges
    printf "\e[2;5H\e[1;101;37m ↖ (1,1) TOP-LEFT \e[0m"
    printf "\e[2;%dH\e[1;101;37m (1,%d) TOP-RIGHT ↗ \e[0m" "$((cols - 20))" "$cols"

    mid_r=$((lines / 2))
    mid_msg=" VELOX FULL MATRIX GRID [ %d ROWS x %d COLS ] " "$lines" "$cols"
    mid_msg_str=$(printf "$mid_msg")
    mid_c=$(( (cols - ${#mid_msg_str}) / 2 ))
    if [ $mid_c -lt 1 ]; then mid_c=1; fi
    printf "\e[%d;%dH\e[1;42;30m%s\e[0m" "$mid_r" "$mid_c" "$mid_msg_str"

    hint_r=$((lines - 2))
    hint_msg=" Press [ESC] or [q] to exit grid test "
    hint_c=$(( (cols - ${#hint_msg}) / 2 ))
    if [ $hint_c -lt 1 ]; then hint_c=1; fi
    printf "\e[%d;%dH\e[1;43;30m%s\e[0m" "$hint_r" "$hint_c" "$hint_msg"

    bot_r=$((lines - 1))
    printf "\e[%d;5H\e[1;101;37m ↙ (%d,1) BOTTOM-LEFT \e[0m" "$bot_r" "$lines"
    printf "\e[%d;%dH\e[1;40;37m (%d,%d) BOTTOM-RIGHT ↘ \e[0m" "$bot_r" "$((cols - 24))" "$lines" "$cols"

    # Wait up to 5 seconds or exit immediately if ESC ('\e') or 'q' is pressed
    read -t 5 -n 1 -r -s key
    if [[ "$key" == $'\e' ]] || [[ "$key" == "q" ]] || [[ "$key" == "Q" ]]; then
        : # ESC pressed, cancel immediately
    fi

    printf "\e[?1049l\e[?25h"
fi

if [ "$SCROLLBACK_TEST" = true ]; then
    echo "14. SCROLLBACK BUFFER STRESS TEST"
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
