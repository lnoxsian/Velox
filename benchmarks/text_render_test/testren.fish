#!/usr/bin/env fish

set scrollback_test false
for arg in $argv
    if test "$arg" = "--scroll-back"
        set scrollback_test true
    end
end

clear

echo "==========================================="
echo "VELOX FONT RENDERING & TEXT ATTRIBUTE BENCHMARK"
echo "==========================================="

sleep 1

#################################################
echo
echo "1. ASCII"
echo

for i in (seq 32 126)
    printf "%b " (printf "\\%03o" $i)
end

echo
sleep 0.5

#################################################
echo
echo "2. TEXT ATTRIBUTES & STYLES"
echo

printf "Normal:          Hello Velox Terminal\n"
printf "\e[1mBold:\e[0m            Hello Velox Terminal\n"
printf "\e[2mDim:\e[0m             Hello Velox Terminal\n"
printf "\e[3mItalic:\e[0m          Hello Velox Terminal\n"
printf "\e[4mUnderline:\e[0m       Hello Velox Terminal\n"
printf "\e[5mBlink:\e[0m           Hello Velox Terminal\n"
printf "\e[7mReverse:\e[0m         Hello Velox Terminal\n"
printf "\e[9mStrikethrough:\e[0m   Hello Velox Terminal\n"
printf "\e[1;3;4;9mCombined (Bold+Italic+Underline+Strikethrough):\e[0m Hello Velox\n"

sleep 0.5

#################################################
echo
echo "3. BOX DRAWING ALIGNMENT GRID"
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
echo "4. BLOCK ELEMENTS"
echo

echo "█▓▒░ ░▒▓█"
echo "▁▂▃▄▅▆▇█"
echo "█▇▆▅▄▃▂▁"

sleep 0.5

#################################################
echo
echo "5. BRAILLE"
echo

echo "⠁⠃⠇⠏⠟⠿⡿⣿ ⣿⡿⠿⠟⠏⠇⠃⠁"

sleep 0.5

#################################################
echo
echo "6. POWERLINE & NERD FONTS"
echo

echo "Powerline:         "
echo "Nerd Icons: 󰣇 󰆍 󰙯 󰈔 󰘚 󰊠"

sleep 0.5

#################################################
echo
echo "7. EMOJIS & FLAGS"
echo

echo "Emojis: 😀 😁 😂 🤣 😃 😄 😅 😆 😉 😊 😍 🥳 🤖 🚀 🎈 🎉 🦄 🦊"
echo "Flags:  🇺🇸 🇯🇵 🇮🇳 🇫🇷 🇬🇧 🇩🇪 🇨🇦 🇦🇺 🇧🇷 🇪🇸 🇮🇹 🇨🇳"

sleep 0.5

#################################################
echo
echo "8. MIXED WIDTH LAYOUT"
echo

echo "Alternating: A中B文C国D語E"
echo "Emojis mixed: Hello 🚀 World! 🤖 Hello 🇨🇳 CJK: 日本語"

sleep 0.5

#################################################
echo
echo "9. COMBINING CHARACTERS"
echo

echo "Standard: á é í ó ú"
echo "Zalgotext: Z͑̄͆ͭ̒̅"

sleep 0.5

#################################################
echo
echo "10. INTERNATIONAL ALPHABETS"
echo

echo "CJK:      日本語 漢字 한국어 中文"
echo "Cyrillic: Привет, как дела? (Russian)"
echo "Greek:    Καλημέρα, τι κάνετε; (Greek)"
echo "Hindi:    नमस्ते दुनिया (Devanagari)"
echo "Arabic:   السلام عليكم (Arabic)"

sleep 0.5

#################################################
echo
echo "11. TRUECOLOR GRADIENTS"
echo
echo "256 Color Ramp:"

for i in (seq 0 255)
    printf "\e[48;5;%sm " "$i"
end
printf "\e[0m\n\n"

echo "24-bit True Color RGB Gradient:"
for r in (seq 0 16 255)
    for g in (seq 0 16 255)
        printf "\e[48;2;%s;%s;64m " "$r" "$g"
    end
    printf "\e[0m\n"
end
printf "\e[0m\n"

sleep 0.5

#################################################
echo
echo "12. SGR COLON SUB-PARAMETERS & SCROLLBACK STRESS"
echo

printf "\e[4:1mUnderline Style 1 (Single) via Colons\e[0m\n"
printf "\e[4:2mUnderline Style 2 (Double) via Colons\e[0m\n"
printf "\e[4:3mUnderline Style 3 (Curly/Underline) via Colons\e[0m\n"
printf "\e[38:2::255:128:0mTruecolor Foreground (Orange) via Colons\e[0m\n"
printf "\e[48:2::0:128:255mTruecolor Background (Blue) via Colons\e[0m\n"

if test "$scrollback_test" = true
    sleep 0.5
    echo
    echo "Generating 1,500 lines to stress test scrollback buffer..."
    sleep 0.5

    for i in (seq 1 1500)
        echo "Scrollback line #$i - testing scrollback memory and limits"
    end

    echo "Scrollback buffer populated! You can scroll up to view the history."
    sleep 1
else
    echo "(Scrollback stress test skipped. Run with --scroll-back to test.)"
    sleep 0.5
end

#################################################
echo
echo "14. PROGRESS BARS & SPINNERS"
echo

set spinner "⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏"
set spinner_len (count $spinner)

tput civis

for i in (seq 0 4 100)
    set filled (math "floor($i / 4)")
    set empty (math "25 - $filled")
    
    set bar ""
    if test $filled -gt 0
        for k in (seq 1 $filled)
            set bar "$bar""█"
        end
    end
    if test $empty -gt 0
        for k in (seq 1 $empty)
            set bar "$bar""░"
        end
    end
    
    set sp_idx (math "($i / 4) % $spinner_len + 1")
    set sp_frame $spinner[$sp_idx]
    
    set r_val (math "255 - $i * 2")
    set g_val (math "$i * 2")
    set b_val (math "floor($i * 255 / 100)")
    
    printf "\r %s \e[1mLoading:\e[0m \e[38;2;%s;%s;%sm%s\e[0m %3d%% " $sp_frame $r_val $g_val $b_val $bar $i
    sleep 0.04
end

tput cnorm
echo
echo

# APT-GET Install Style Progress Bar
echo "APT-GET Install Style Progress Bar:"
echo "Selecting previously unselected package velox-terminal..."
echo "Preparing to unpack .../velox-terminal_0.1.0_amd64.deb ..."
echo "Unpacking velox-terminal (0.1.0) ..."
echo "Setting up velox-terminal (0.1.0) ..."

tput civis
for i in (seq 0 5 100)
    set filled (math "floor($i / 2)")
    set empty (math "50 - $filled")
    
    set bar ""
    if test $filled -gt 0
        for k in (seq 1 $filled)
            set bar "$bar""#"
        end
    end
    if test $empty -gt 0
        for k in (seq 1 $empty)
            set bar "$bar""."
        end
    end
    
    printf "\rProgress: [\e[32m%3d%%\e[0m] [\e[32m%s\e[0m%s]" $i $bar ""
    sleep 0.04
end
tput cnorm
echo
echo

# DNF Install Style Progress Bar
echo "DNF Install Style Progress Bar:"
echo "Downloading Packages:"
echo "velox-terminal-0.1.0-1.fc40.x86_64.rpm           |  12 MB/s |  15 MB     00:01"
echo "Installing:"

tput civis
for i in (seq 0 5 100)
    set filled (math "floor($i / 4)")
    set empty (math "25 - $filled")
    
    set bar ""
    if test $filled -gt 0
        for k in (seq 1 $filled)
            set bar "$bar""="
        end
    end
    if test $filled -lt 25
        set bar "$bar"">"
        set empty (math "$empty - 1")
    end
    if test $empty -gt 0
        for k in (seq 1 $empty)
            set bar "$bar"" "
        end
    end
    
    printf "\rvelox-terminal-0.1.0-1.fc40.x86_64               [\e[36m%s\e[0m] %3d%%" $bar $i
    sleep 0.04
end
tput cnorm
echo
echo

# Pacman Style Eating-Pacman Progress Bar
echo "Pacman (Arch) Style Progress Bar:"
tput civis
for i in (seq 0 4 100)
    set filled (math "floor($i / 4)")
    set empty (math "25 - $filled")
    
    set bar ""
    if test $filled -gt 0
        for k in (seq 1 $filled)
            set bar "$bar""#"
        end
    end
    
    set pacman_char "C"
    if test (math "($i / 4) % 2") -eq 0
        set pacman_char "c"
    end
    
    if test $filled -lt 25
        set bar "$bar""\e[33m$pacman_char\e[0m"
        
        set food ""
        set empty_food (math "$empty - 1")
        if test $empty_food -gt 0
            for k in (seq 1 $empty_food)
                if test (math "$k % 2") -eq 0
                    set food "$food""-"
                else
                    set food "$food"" "
                end
            end
        end
        set bar "$bar""$food"
    end
    
    printf "\rvelox-terminal-0.1.0-1-x86_64      14.3 MiB  12.4 MiB/s 00:01 [\e[34m%b\e[0m] %3d%%" $bar $i
    sleep 0.04
end
tput cnorm
echo
echo "Done!"
sleep 0.5

#################################################
echo
echo "13. CURSOR TOGGLING"
echo

tput civis

for i in (seq 1 20)
    printf "\rUpdating frame %d/20..." "$i"
    sleep 0.05
end

tput cnorm

echo
echo "==========================================="
echo "BENCHMARK COMPLETED"
echo "==========================================="
echo
