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
