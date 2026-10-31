#!/usr/bin/env bash

SCROLLBACK_ONLY=false
GRID_ONLY=false
BOX_ONLY=false

for arg in "$@"; do
    if [ "$arg" = "--scroll-back" ] || [ "$arg" = "--scrollback" ]; then
        SCROLLBACK_ONLY=true
    fi
    if [ "$arg" = "--grid" ] || [ "$arg" = "--full-grid" ]; then
        GRID_ONLY=true
    fi
    if [ "$arg" = "--box" ] || [ "$arg" = "--box-only" ] || [ "$arg" = "--box-draw" ]; then
        BOX_ONLY=true
    fi
done

clear

run_box_drawing_tests() {
    echo "================================================================================"
    echo "4. UNICODE BOX DRAWING COMPREHENSIVE BENCHMARK (U+2500 - U+257F)"
    echo "   Reference: https://www.w3schools.com/charsets/ref_utf_box.asp"
    echo "   Full Set of 128 Unicode Box Drawing Characters (Single, Double, Heavy, Dashed, Hybrids)"
    echo "================================================================================"
    echo
    echo "--- 4.0: COMPLETE 128-GLYPH CODEPOINT MATRIX (U+2500 - U+257F) ---"
    echo
    cat << 'EOF'
       +0 +1 +2 +3 +4 +5 +6 +7 +8 +9 +A +B +C +D +E +F
U+2500: ─  ━  │  ┃  ┄  ┅  ┆  ┇  ┈  ┉  ┊  ┋  ┌  ┍  ┎  ┏
U+2510: ┐  ┑  ┒  ┓  └  ┕  ┖  ┗  ┘  ┙  ┚  ┛  ├  ┝  ┞  ┟
U+2520: ┠  ┡  ┢  ┣  ┤  ┥  ┦  ┧  ┨  ┩  ┪  ┫  ┬  ┭  ┮  ┯
U+2530: ┰  ┱  ┲  ┳  ┴  ┵  ┶  ┷  ┸  ┹  ┺  ┻  ┼  ┽  ┾  ┿
U+2540: ╀  ╁  ╂  ╃  ╄  ╅  ╆  ╇  ╈  ╉  ╊  ╋  ╌  ╍  ╎  ╏
U+2550: ═  ║  ╒  ╓  ╔  ╕  ╖  ╗  ╘  ╙  ╚  ╛  ╜  ╝  ╞  ╟
U+2560: ╠  ╡  ╢  ╣  ╤  ╥  ╦  ╧  ╨  ╩  ╪  ╫  ╬  ╭  ╮  ╯
U+2570: ╰  ╱  ╲  ╳  ╴  ╵  ╶  ╷  ╸  ╹  ╺  ╻  ╼  ╽  ╾  ╿
EOF
    echo
    echo "--- 4.1: PRIMARY BOX FRAMES & 3×3 MATRIX GRIDS ---"
    echo
    cat << 'EOF'
[ Light / Single ]     [ Heavy / Bold ]       [ Double (Concentric) ]  [ Rounded / Arc (Single) ]
┌───┬───┬───┐          ┏━━━┳━━━┳━━━┓          ╔═══╦═══╦═══╗            ╭───┬───┬───╮
│ 1 │ 2 │ 3 │          ┃ 1 ┃ 2 ┃ 3 ┃          ║ 1 ║ 2 ║ 3 ║            │ 1 │ 2 │ 3 │
├───┼───┼───┤          ┣━━━╋━━━╋━━━┫          ╠═══╬═══╬═══╣            ├───┼───┼───┤
│ 4 │ 5 │ 6 │          ┃ 4 ┃ 5 ┃ 6 ┃          ║ 4 ║ 5 ║ 6 ║            │ 4 │ 5 │ 6 │
├───┼───┼───┤          ┣━━━╋━━━╋━━━┫          ╠═══╬═══╬═══╣            ├───┼───┼───┤
│ 7 │ 8 │ 9 │          ┃ 7 ┃ 8 ┃ 9 ┃          ║ 7 ║ 8 ║ 9 ║            │ 7 │ 8 │ 9 │
└───┴───┴───┘          ┗━━━┻━━━┻━━━┛          ╚═══╩═══╩═══╝            ╰───┴───┴───╯

[ Square vs. Smooth Curved Corners Comparison ]
  Single Square (┌ ┐ ┘ └):      Single Smooth Curve (╭ ╮ ╯ ╰):
  ┌──────────────────────┐      ╭──────────────────────╮
  │ Sharp 90° Orthogonal │      │ Smooth Curved Arc    │
  └──────────────────────┘      ╰──────────────────────╯

  Double Square/Standard:       Double Smooth Concentric (╔ ╗ ╝ ╚):
  ┌──╥────────────────╥──┐      ╔══════════════════════╗
  │  ║ (Orthogonal)   ║  │      ║ Smooth Double Curves ║
  └──╨────────────────╨──┘      ╚══════════════════════╝
EOF
    echo
    echo "--- 4.2: DASHED BOXES (TRIPLE, QUADRUPLE & DOUBLE DASH - LIGHT & HEAVY) ---"
    echo
    cat << 'EOF'
[ Light Triple Dash ]     [ Heavy Triple Dash ]     [ Light Quadruple Dash ]  [ Heavy Quadruple Dash ]
┌┄┄┄┬┄┄┄┐                 ┏┅┅┅┳┅┅┅┓                 ┌┈┈┈┬┈┈┈┐                 ┏┉┉┉┳┉┉┉┓
┆ 1 ┆ 2 ┆                 ┇ 1 ┇ 2 ┇                 ┊ 1 ┊ 2 ┊                 ┋ 1 ┋ 2 ┋
├┄┄┄┼┄┄┄┤                 ┣┅┅┅╋┅┅┅┫                 ├┈┈┈┼┈┈┈┤                 ┣┉┉┉╋┉┉┉┫
┆ 3 ┆ 4 ┆                 ┇ 3 ┇ 4 ┇                 ┊ 3 ┊ 4 ┊                 ┋ 3 ┋ 4 ┋
└┄┄┄┴┄┄┄┘                 ┗┅┅┅┻┅┅┅┛                 └┈┈┈┴┈┈┈┘                 ┗┉┉┉┻┉┉┉┛

[ Light Double Dash ]     [ Heavy Double Dash ]     [ Mixed Dashed Outer + Solid Inner ]
┌╌╌╌┬╌╌╌┐                 ┏╍╍╍┳╍╍╍┓                 ┌┄┄┄┬┄┄┄┐
╎ 1 ╎ 2 ╎                 ╏ 1 ╏ 2 ╏                 ┆ 1 │ 2 ┆
├╌╌╌┼╌╌╌┤                 ┣╍╍╍╋╍╍╍┫                 ├───┼───┤
╎ 3 ╎ 4 ╎                 ╏ 3 ╏ 4 ╏                 ┆ 3 │ 4 ┆
└╌╌╌┴╌╌╌┘                 ┗╍╍╍┻╍╍╍┛                 └┄┄┄┴┄┄┄┘
EOF
    echo
    echo "--- 4.3: HYBRID SINGLE & DOUBLE INTERSECTIONS ---"
    echo
    cat << 'EOF'
[ Double Horiz / Single Vert ]      [ Single Horiz / Double Vert ]      [ Double Outer / Single Inner ]
╒═════════╤═════════╕               ╓─────────╥─────────╖               ╔═════════╤═════════╗
│ U+2552  │ U+2564  │ U+2555        ║ U+2553  ║ U+2565  ║ U+2556        ║ Outer D │ Outer D ║
╞═════════╪═════════╡               ╟─────────╫─────────╢               ╟─────────┼─────────╢
│ U+255E  │ U+256A  │ U+2561        ║ U+255F  ║ U+256B  ║ U+2562        ║ Inner S │ Inner S ║
╘═════════╧═════════╛               ╙─────────╨─────────╜               ╚═════════╧═════════╝

[ Single Outer / Double Inner ]     [ Complex Single/Double Cross Combination Matrix ]
┌─────────╥─────────┐               ╒═════╤═════╕  ╓─────╥─────╖  ╔═════╤═════╗
│ Outer S ║ Outer S │               │  A  │  B  │  ║  C  ║  D  ║  ║  E  │  F  ║
╞═════════╬═════════╡               ╞═════╪═════╡  ╟─────╫─────╢  ╠═════╪═════╣
│ Inner D ║ Inner D │               │  G  │  H  │  ║  I  ║  J  ║  ║  K  │  L  ║
└─────────╨─────────┘               ╘═════╧═════╛  ╙─────╨─────╜  ╚═════╧═════╝
EOF
    echo
    echo "--- 4.4: MIXED LIGHT & HEAVY WEIGHT INTERSECTIONS (ALL 36 HYBRIDS) ---"
    echo
    cat << 'EOF'
[ Heavy Horiz / Light Vert ]        [ Light Horiz / Heavy Vert ]        [ Heavy Frame / Light Grid ]
┍━━━━━━━━━┯━━━━━━━━━┑               ┎─────────┰─────────┒               ┏━━━━━━━━━┳━━━━━━━━━┓
│ U+250D  │ U+252F  │ U+2511        ┃ U+250E  ┃ U+2530  ┃ U+2512        ┃ H-Top   ┃ H-Top   ┃
┝━━━━━━━━━┿━━━━━━━━━┥               ┠─────────╂─────────┨               ┣───┼───┼─╋───┼───┼─┫
│ U+251D  │ U+253F  │ U+2525        ┃ U+2520  ┃ U+2542  ┃ U+2528        ┃ Light   ┃ Light   ┃
┕━━━━━━━━━┷━━━━━━━━━┙               ┖─────────┸─────────┚               ┗━━━━━━━━━┻━━━━━━━━━┛

[ Asymmetric Tees: Left/Right Heavy ]  [ Asymmetric Tees: Up/Down Heavy ]  [ Complex 4-Way Crosses ]
┍━━━┭───┮━━━┑  ┌───┱───┲───┐        ┎───┰───┒  ┍━━━┳━━━┑               ┌───┽───┾───┐
│ A │ B │ C │  │ D │ E │ F │        ┃ G ┃ H ┃  │ I ┃ J │               │ U+253D/E  │
┞───┽───┾───┦  ┝━━━┿━━━┿━━━┥        ┠───╂───┨  ┞───╀───┦               ├───╀───╁───┤
│ G │ H │ I │  │ J │ K │ L │        ┃ M ┃ N ┃  │ O ┃ P │               │ U+2540/1  │
┟───┿───╀───┧  ┖───┹───┺───┚        ┡━━━╅───╆  ┢━━━╃───╄               ├───╃───╄───┤
│ M │ N │ O │  ┌───┵───┶───┐        │ Q │ R │  ┃ S │ T │               │ U+2543/4  │
┕━━━┵───┶━━━┙  └───┴───┴───┘        ┗━━━┻━━━┛  ┗━━━┻━━━┛               └───┴───┴───┘

[ Asymmetric Cross Matrix (U+2543 - U+254B) ]
  ╃ (L-Up-Hvy, R-Dn-Lt)   ╄ (R-Up-Hvy, L-Dn-Lt)   ╅ (L-Dn-Hvy, R-Up-Lt)   ╆ (R-Dn-Hvy, L-Up-Lt)
  ╇ (Dn-Lt, Up-Horiz-Hvy) ╈ (Up-Lt, Dn-Horiz-Hvy) ╉ (R-Lt, L-Vert-Hvy)    ╊ (L-Lt, R-Vert-Hvy)
  ╋ (Full Heavy Cross)    ╂ (Vert Heavy, Horiz Light)
EOF
    echo
    echo "--- 4.5: DIAGONAL ELEMENTS & BOX HATCHING (U+2571 - U+2573) ---"
    echo
    cat << 'EOF'
[ Diagonal Slashing & Cross Frames ]              [ Diamond / Patterned Tessellation ]
┌───────────────┬───────────────┐                 ┌───┬───┬───┬───┐
│ ╲           ╱ │ ╱ ╱ ╱ ╱ ╱ ╱ ╱ │                 │ ╲ │ ╱ │ ╲ │ ╱ │
│   ╲       ╱   │ ╱ ╱ ╱ ╱ ╱ ╱ ╱ │                 │  ╲│╱  │  ╲│╱  │
│     ╲   ╱     │ ╱ ╱ ╱ ╱ ╱ ╱ ╱ │                 ├───╳───┼───╳───┤
│       ╳       │ ╱ ╱ ╱ ╱ ╱ ╱ ╱ │                 │  ╱│╲  │  ╱│╲  │
│     ╱   ╲     │ ╲ ╲ ╲ ╲ ╲ ╲ ╲ │                 │ ╱ │ ╲ │ ╱ │ ╲ │
│   ╱       ╲   │ ╲ ╲ ╲ ╲ ╲ ╲ ╲ │                 └───┴───┴───┴───┘
│ ╱           ╲ │ ╳ ╳ ╳ ╳ ╳ ╳ ╳ │                 Diagonal glyphs: ╱ (U+2571)  ╲ (U+2572)  ╳ (U+2573)
└───────────────┴───────────────┘
EOF
    echo
    echo "--- 4.6: LINE CAPS, TERMINAL STUBS & WEIGHT TRANSITIONS (U+2574 - U+257F) ---"
    echo
    cat << 'EOF'
[ Isolated Stubs & Caps (Light vs Heavy) ]        [ Weight Transitions (Light ↔ Heavy) ]
Light Left:      [ ────╴ ]  (U+2574)              Light-Left / Heavy-Right: [ ────╼━━━━ ]  (U+257C)
Light Up:        [   ╵   ]  (U+2575)              Light-Up / Heavy-Down:    [   ╽   ]  (U+257D)
Light Right:     [ ╶──── ]  (U+2576)                                        [   ┃   ]  
Light Down:      [   ╷   ]  (U+2577)              Heavy-Left / Light-Right: [ ━━━━╾──── ]  (U+257E)
Heavy Left:      [ ━━━━╸ ]  (U+2578)              Heavy-Up / Light-Down:    [   ┃   ]  
Heavy Up:        [   ╹   ]  (U+2579)                                        [   ╿   ]  (U+257F)
Heavy Right:     [ ╺━━━━ ]  (U+257A)                                        [   │   ]  
Heavy Down:      [   ╻   ]  (U+257B)

[ Star & Crosshair Alignment Targets ]
       ╵                 ╹                 ╽                 ╿
       │                 ┃                 │                 ┃
   ╶───┼───╴         ╺━━━╋━━━╸         ╾───┼───╼         ╺━━━╂━━━╸
       │                 ┃                 ┃                 │
       ╷                 ╻                 ╿                 ╽
EOF
    echo
    echo "--- 4.7: COMPLETE W3SCHOOLS UTF-8 BOX DRAWING REFERENCE DIRECTORY (128 CHARACTERS) ---"
    echo
    printf "\e[1;34m=== 1. Standard & Double Lines (6 Characters) ===\e[0m\n"
    printf "  \e[1;36mU+2500\e[0m (\e[1;33m9472\e[0m)  \e[1;32m─\e[0m  Light Horizontal\n"
    printf "  \e[1;36mU+2501\e[0m (\e[1;33m9473\e[0m)  \e[1;32m━\e[0m  Heavy Horizontal\n"
    printf "  \e[1;36mU+2502\e[0m (\e[1;33m9474\e[0m)  \e[1;32m│\e[0m  Light Vertical\n"
    printf "  \e[1;36mU+2503\e[0m (\e[1;33m9475\e[0m)  \e[1;32m┃\e[0m  Heavy Vertical\n"
    printf "  \e[1;36mU+2550\e[0m (\e[1;33m9552\e[0m)  \e[1;32m═\e[0m  Double Horizontal\n"
    printf "  \e[1;36mU+2551\e[0m (\e[1;33m9553\e[0m)  \e[1;32m║\e[0m  Double Vertical\n"
    echo
    printf "\e[1;34m=== 2. Dashed Lines (Triple, Quadruple & Double Dash) (12 Characters) ===\e[0m\n"
    printf "  \e[1;36mU+2504\e[0m (\e[1;33m9476\e[0m)  \e[1;32m┄\e[0m  Light Triple Dash Horizontal\n"
    printf "  \e[1;36mU+2505\e[0m (\e[1;33m9477\e[0m)  \e[1;32m┅\e[0m  Heavy Triple Dash Horizontal\n"
    printf "  \e[1;36mU+2506\e[0m (\e[1;33m9478\e[0m)  \e[1;32m┆\e[0m  Light Triple Dash Vertical\n"
    printf "  \e[1;36mU+2507\e[0m (\e[1;33m9479\e[0m)  \e[1;32m┇\e[0m  Heavy Triple Dash Vertical\n"
    printf "  \e[1;36mU+2508\e[0m (\e[1;33m9480\e[0m)  \e[1;32m┈\e[0m  Light Quadruple Dash Horizontal\n"
    printf "  \e[1;36mU+2509\e[0m (\e[1;33m9481\e[0m)  \e[1;32m┉\e[0m  Heavy Quadruple Dash Horizontal\n"
    printf "  \e[1;36mU+250A\e[0m (\e[1;33m9482\e[0m)  \e[1;32m┊\e[0m  Light Quadruple Dash Vertical\n"
    printf "  \e[1;36mU+250B\e[0m (\e[1;33m9483\e[0m)  \e[1;32m┋\e[0m  Heavy Quadruple Dash Vertical\n"
    printf "  \e[1;36mU+254C\e[0m (\e[1;33m9548\e[0m)  \e[1;32m╌\e[0m  Light Double Dash Horizontal\n"
    printf "  \e[1;36mU+254D\e[0m (\e[1;33m9549\e[0m)  \e[1;32m╍\e[0m  Heavy Double Dash Horizontal\n"
    printf "  \e[1;36mU+254E\e[0m (\e[1;33m9550\e[0m)  \e[1;32m╎\e[0m  Light Double Dash Vertical\n"
    printf "  \e[1;36mU+254F\e[0m (\e[1;33m9551\e[0m)  \e[1;32m╏\e[0m  Heavy Double Dash Vertical\n"
    echo
    printf "\e[1;34m=== 3. Corners & Arcs (Light, Heavy, Double & Rounded) (32 Characters) ===\e[0m\n"
    printf "  \e[1;36mU+250C\e[0m (\e[1;33m9484\e[0m)  \e[1;32m┌\e[0m  Light Down & Right\n"
    printf "  \e[1;36mU+250D\e[0m (\e[1;33m9485\e[0m)  \e[1;32m┍\e[0m  Down Light & Right Heavy\n"
    printf "  \e[1;36mU+250E\e[0m (\e[1;33m9486\e[0m)  \e[1;32m┎\e[0m  Down Heavy & Right Light\n"
    printf "  \e[1;36mU+250F\e[0m (\e[1;33m9487\e[0m)  \e[1;32m┏\e[0m  Heavy Down & Right\n"
    printf "  \e[1;36mU+2510\e[0m (\e[1;33m9488\e[0m)  \e[1;32m┐\e[0m  Light Down & Left\n"
    printf "  \e[1;36mU+2511\e[0m (\e[1;33m9489\e[0m)  \e[1;32m┑\e[0m  Down Light & Left Heavy\n"
    printf "  \e[1;36mU+2512\e[0m (\e[1;33m9490\e[0m)  \e[1;32m┒\e[0m  Down Heavy & Left Light\n"
    printf "  \e[1;36mU+2513\e[0m (\e[1;33m9491\e[0m)  \e[1;32m┓\e[0m  Heavy Down & Left\n"
    printf "  \e[1;36mU+2514\e[0m (\e[1;33m9492\e[0m)  \e[1;32m└\e[0m  Light Up & Right\n"
    printf "  \e[1;36mU+2515\e[0m (\e[1;33m9493\e[0m)  \e[1;32m┕\e[0m  Up Light & Right Heavy\n"
    printf "  \e[1;36mU+2516\e[0m (\e[1;33m9494\e[0m)  \e[1;32m┖\e[0m  Up Heavy & Right Light\n"
    printf "  \e[1;36mU+2517\e[0m (\e[1;33m9495\e[0m)  \e[1;32m┗\e[0m  Heavy Up & Right\n"
    printf "  \e[1;36mU+2518\e[0m (\e[1;33m9496\e[0m)  \e[1;32m┘\e[0m  Light Up & Left\n"
    printf "  \e[1;36mU+2519\e[0m (\e[1;33m9497\e[0m)  \e[1;32m┙\e[0m  Up Light & Left Heavy\n"
    printf "  \e[1;36mU+251A\e[0m (\e[1;33m9498\e[0m)  \e[1;32m┚\e[0m  Up Heavy & Left Light\n"
    printf "  \e[1;36mU+251B\e[0m (\e[1;33m9499\e[0m)  \e[1;32m┛\e[0m  Heavy Up & Left\n"
    printf "  \e[1;36mU+2552\e[0m (\e[1;33m9554\e[0m)  \e[1;32m╒\e[0m  Down Single & Right Double\n"
    printf "  \e[1;36mU+2553\e[0m (\e[1;33m9555\e[0m)  \e[1;32m╓\e[0m  Down Double & Right Single\n"
    printf "  \e[1;36mU+2554\e[0m (\e[1;33m9556\e[0m)  \e[1;32m╔\e[0m  Double Down & Right\n"
    printf "  \e[1;36mU+2555\e[0m (\e[1;33m9557\e[0m)  \e[1;32m╕\e[0m  Down Single & Left Double\n"
    printf "  \e[1;36mU+2556\e[0m (\e[1;33m9558\e[0m)  \e[1;32m╖\e[0m  Down Double & Left Single\n"
    printf "  \e[1;36mU+2557\e[0m (\e[1;33m9559\e[0m)  \e[1;32m╗\e[0m  Double Down & Left\n"
    printf "  \e[1;36mU+2558\e[0m (\e[1;33m9560\e[0m)  \e[1;32m╘\e[0m  Up Single & Right Double\n"
    printf "  \e[1;36mU+2559\e[0m (\e[1;33m9561\e[0m)  \e[1;32m╙\e[0m  Up Double & Right Single\n"
    printf "  \e[1;36mU+255A\e[0m (\e[1;33m9562\e[0m)  \e[1;32m╚\e[0m  Double Up & Right\n"
    printf "  \e[1;36mU+255B\e[0m (\e[1;33m9563\e[0m)  \e[1;32m╛\e[0m  Up Single & Left Double\n"
    printf "  \e[1;36mU+255C\e[0m (\e[1;33m9564\e[0m)  \e[1;32m╜\e[0m  Up Double & Left Single\n"
    printf "  \e[1;36mU+255D\e[0m (\e[1;33m9565\e[0m)  \e[1;32m╝\e[0m  Double Up & Left\n"
    printf "  \e[1;36mU+256D\e[0m (\e[1;33m9581\e[0m)  \e[1;32m╭\e[0m  Light Arc Down & Right\n"
    printf "  \e[1;36mU+256E\e[0m (\e[1;33m9582\e[0m)  \e[1;32m╮\e[0m  Light Arc Down & Left\n"
    printf "  \e[1;36mU+256F\e[0m (\e[1;33m9583\e[0m)  \e[1;32m╯\e[0m  Light Arc Up & Left\n"
    printf "  \e[1;36mU+2570\e[0m (\e[1;33m9584\e[0m)  \e[1;32m╰\e[0m  Light Arc Up & Right\n"
    echo
    printf "\e[1;34m=== 4. T-Junctions / Tees (Vertical & Horizontal) (44 Characters) ===\e[0m\n"
    printf "  \e[1;36mU+251C\e[0m (\e[1;33m9500\e[0m)  \e[1;32m├\e[0m  Light Vertical & Right\n"
    printf "  \e[1;36mU+251D\e[0m (\e[1;33m9501\e[0m)  \e[1;32m┝\e[0m  Vertical Light & Right Heavy\n"
    printf "  \e[1;36mU+251E\e[0m (\e[1;33m9502\e[0m)  \e[1;32m┞\e[0m  Up Heavy & Right Down Light\n"
    printf "  \e[1;36mU+251F\e[0m (\e[1;33m9503\e[0m)  \e[1;32m┟\e[0m  Down Heavy & Right Up Light\n"
    printf "  \e[1;36mU+2520\e[0m (\e[1;33m9504\e[0m)  \e[1;32m┠\e[0m  Vertical Heavy & Right Light\n"
    printf "  \e[1;36mU+2521\e[0m (\e[1;33m9505\e[0m)  \e[1;32m┡\e[0m  Down Light & Right Up Heavy\n"
    printf "  \e[1;36mU+2522\e[0m (\e[1;33m9506\e[0m)  \e[1;32m┢\e[0m  Up Light & Right Down Heavy\n"
    printf "  \e[1;36mU+2523\e[0m (\e[1;33m9507\e[0m)  \e[1;32m┣\e[0m  Heavy Vertical & Right\n"
    printf "  \e[1;36mU+2524\e[0m (\e[1;33m9508\e[0m)  \e[1;32m┤\e[0m  Light Vertical & Left\n"
    printf "  \e[1;36mU+2525\e[0m (\e[1;33m9509\e[0m)  \e[1;32m┥\e[0m  Vertical Light & Left Heavy\n"
    printf "  \e[1;36mU+2526\e[0m (\e[1;33m9510\e[0m)  \e[1;32m┦\e[0m  Up Heavy & Left Down Light\n"
    printf "  \e[1;36mU+2527\e[0m (\e[1;33m9511\e[0m)  \e[1;32m┧\e[0m  Down Heavy & Left Up Light\n"
    printf "  \e[1;36mU+2528\e[0m (\e[1;33m9512\e[0m)  \e[1;32m┨\e[0m  Vertical Heavy & Left Light\n"
    printf "  \e[1;36mU+2529\e[0m (\e[1;33m9513\e[0m)  \e[1;32m┩\e[0m  Down Light & Left Up Heavy\n"
    printf "  \e[1;36mU+252A\e[0m (\e[1;33m9514\e[0m)  \e[1;32m┪\e[0m  Up Light & Left Down Heavy\n"
    printf "  \e[1;36mU+252B\e[0m (\e[1;33m9515\e[0m)  \e[1;32m┫\e[0m  Heavy Vertical & Left\n"
    printf "  \e[1;36mU+252C\e[0m (\e[1;33m9516\e[0m)  \e[1;32m┬\e[0m  Light Down & Horizontal\n"
    printf "  \e[1;36mU+252D\e[0m (\e[1;33m9517\e[0m)  \e[1;32m┭\e[0m  Left Heavy & Right Down Light\n"
    printf "  \e[1;36mU+252E\e[0m (\e[1;33m9518\e[0m)  \e[1;32m┮\e[0m  Right Heavy & Left Down Light\n"
    printf "  \e[1;36mU+252F\e[0m (\e[1;33m9519\e[0m)  \e[1;32m┯\e[0m  Down Light & Horizontal Heavy\n"
    printf "  \e[1;36mU+2530\e[0m (\e[1;33m9520\e[0m)  \e[1;32m┰\e[0m  Down Heavy & Horizontal Light\n"
    printf "  \e[1;36mU+2531\e[0m (\e[1;33m9521\e[0m)  \e[1;32m┱\e[0m  Right Light & Left Down Heavy\n"
    printf "  \e[1;36mU+2532\e[0m (\e[1;33m9522\e[0m)  \e[1;32m┲\e[0m  Left Light & Right Down Heavy\n"
    printf "  \e[1;36mU+2533\e[0m (\e[1;33m9523\e[0m)  \e[1;32m┳\e[0m  Heavy Down & Horizontal\n"
    printf "  \e[1;36mU+2534\e[0m (\e[1;33m9524\e[0m)  \e[1;32m┴\e[0m  Light Up & Horizontal\n"
    printf "  \e[1;36mU+2535\e[0m (\e[1;33m9525\e[0m)  \e[1;32m┵\e[0m  Left Heavy & Right Up Light\n"
    printf "  \e[1;36mU+2536\e[0m (\e[1;33m9526\e[0m)  \e[1;32m┶\e[0m  Right Heavy & Left Up Light\n"
    printf "  \e[1;36mU+2537\e[0m (\e[1;33m9527\e[0m)  \e[1;32m┷\e[0m  Up Light & Horizontal Heavy\n"
    printf "  \e[1;36mU+2538\e[0m (\e[1;33m9528\e[0m)  \e[1;32m┸\e[0m  Up Heavy & Horizontal Light\n"
    printf "  \e[1;36mU+2539\e[0m (\e[1;33m9529\e[0m)  \e[1;32m┹\e[0m  Right Light & Left Up Heavy\n"
    printf "  \e[1;36mU+253A\e[0m (\e[1;33m9530\e[0m)  \e[1;32m┺\e[0m  Left Light & Right Up Heavy\n"
    printf "  \e[1;36mU+253B\e[0m (\e[1;33m9531\e[0m)  \e[1;32m┻\e[0m  Heavy Up & Horizontal\n"
    printf "  \e[1;36mU+255E\e[0m (\e[1;33m9566\e[0m)  \e[1;32m╞\e[0m  Vertical Single & Right Double\n"
    printf "  \e[1;36mU+255F\e[0m (\e[1;33m9567\e[0m)  \e[1;32m╟\e[0m  Vertical Double & Right Single\n"
    printf "  \e[1;36mU+2560\e[0m (\e[1;33m9568\e[0m)  \e[1;32m╠\e[0m  Double Vertical & Right\n"
    printf "  \e[1;36mU+2561\e[0m (\e[1;33m9569\e[0m)  \e[1;32m╡\e[0m  Vertical Single & Left Double\n"
    printf "  \e[1;36mU+2562\e[0m (\e[1;33m9570\e[0m)  \e[1;32m╢\e[0m  Vertical Double & Left Single\n"
    printf "  \e[1;36mU+2563\e[0m (\e[1;33m9571\e[0m)  \e[1;32m╣\e[0m  Double Vertical & Left\n"
    printf "  \e[1;36mU+2564\e[0m (\e[1;33m9572\e[0m)  \e[1;32m╤\e[0m  Down Single & Horizontal Double\n"
    printf "  \e[1;36mU+2565\e[0m (\e[1;33m9573\e[0m)  \e[1;32m╥\e[0m  Down Double & Horizontal Single\n"
    printf "  \e[1;36mU+2566\e[0m (\e[1;33m9574\e[0m)  \e[1;32m╦\e[0m  Double Down & Horizontal\n"
    printf "  \e[1;36mU+2567\e[0m (\e[1;33m9575\e[0m)  \e[1;32m╧\e[0m  Up Single & Horizontal Double\n"
    printf "  \e[1;36mU+2568\e[0m (\e[1;33m9576\e[0m)  \e[1;32m╨\e[0m  Up Double & Horizontal Single\n"
    printf "  \e[1;36mU+2569\e[0m (\e[1;33m9577\e[0m)  \e[1;32m╩\e[0m  Double Up & Horizontal\n"
    echo
    printf "\e[1;34m=== 5. Crosses & 4-Way Intersections (19 Characters) ===\e[0m\n"
    printf "  \e[1;36mU+253C\e[0m (\e[1;33m9532\e[0m)  \e[1;32m┼\e[0m  Light Vertical & Horizontal\n"
    printf "  \e[1;36mU+253D\e[0m (\e[1;33m9533\e[0m)  \e[1;32m┽\e[0m  Left Heavy & Right Vertical Light\n"
    printf "  \e[1;36mU+253E\e[0m (\e[1;33m9534\e[0m)  \e[1;32m┾\e[0m  Right Heavy & Left Vertical Light\n"
    printf "  \e[1;36mU+253F\e[0m (\e[1;33m9535\e[0m)  \e[1;32m┿\e[0m  Vertical Light & Horizontal Heavy\n"
    printf "  \e[1;36mU+2540\e[0m (\e[1;33m9536\e[0m)  \e[1;32m╀\e[0m  Up Heavy & Down Horizontal Light\n"
    printf "  \e[1;36mU+2541\e[0m (\e[1;33m9537\e[0m)  \e[1;32m╁\e[0m  Down Heavy & Up Horizontal Light\n"
    printf "  \e[1;36mU+2542\e[0m (\e[1;33m9538\e[0m)  \e[1;32m╂\e[0m  Vertical Heavy & Horizontal Light\n"
    printf "  \e[1;36mU+2543\e[0m (\e[1;33m9539\e[0m)  \e[1;32m╃\e[0m  Left Up Heavy & Right Down Light\n"
    printf "  \e[1;36mU+2544\e[0m (\e[1;33m9540\e[0m)  \e[1;32m╄\e[0m  Right Up Heavy & Left Down Light\n"
    printf "  \e[1;36mU+2545\e[0m (\e[1;33m9541\e[0m)  \e[1;32m╅\e[0m  Left Down Heavy & Right Up Light\n"
    printf "  \e[1;36mU+2546\e[0m (\e[1;33m9542\e[0m)  \e[1;32m╆\e[0m  Right Down Heavy & Left Up Light\n"
    printf "  \e[1;36mU+2547\e[0m (\e[1;33m9543\e[0m)  \e[1;32m╇\e[0m  Down Light & Up Horizontal Heavy\n"
    printf "  \e[1;36mU+2548\e[0m (\e[1;33m9544\e[0m)  \e[1;32m╈\e[0m  Up Light & Down Horizontal Heavy\n"
    printf "  \e[1;36mU+2549\e[0m (\e[1;33m9545\e[0m)  \e[1;32m╉\e[0m  Right Light & Left Vertical Heavy\n"
    printf "  \e[1;36mU+254A\e[0m (\e[1;33m9546\e[0m)  \e[1;32m╊\e[0m  Left Light & Right Vertical Heavy\n"
    printf "  \e[1;36mU+254B\e[0m (\e[1;33m9547\e[0m)  \e[1;32m╋\e[0m  Heavy Vertical & Horizontal\n"
    printf "  \e[1;36mU+256A\e[0m (\e[1;33m9578\e[0m)  \e[1;32m╪\e[0m  Vertical Single & Horizontal Double\n"
    printf "  \e[1;36mU+256B\e[0m (\e[1;33m9579\e[0m)  \e[1;32m╫\e[0m  Vertical Double & Horizontal Single\n"
    printf "  \e[1;36mU+256C\e[0m (\e[1;33m9580\e[0m)  \e[1;32m╬\e[0m  Double Vertical & Horizontal\n"
    echo
    printf "\e[1;34m=== 6. Diagonals, Terminal Caps & Weight Transitions (15 Characters) ===\e[0m\n"
    printf "  \e[1;36mU+2571\e[0m (\e[1;33m9585\e[0m)  \e[1;32m╱\e[0m  Light Diagonal Upper Right To Lower Left\n"
    printf "  \e[1;36mU+2572\e[0m (\e[1;33m9586\e[0m)  \e[1;32m╲\e[0m  Light Diagonal Upper Left To Lower Right\n"
    printf "  \e[1;36mU+2573\e[0m (\e[1;33m9587\e[0m)  \e[1;32m╳\e[0m  Light Diagonal Cross\n"
    printf "  \e[1;36mU+2574\e[0m (\e[1;33m9588\e[0m)  \e[1;32m╴\e[0m  Light Left\n"
    printf "  \e[1;36mU+2575\e[0m (\e[1;33m9589\e[0m)  \e[1;32m╵\e[0m  Light Up\n"
    printf "  \e[1;36mU+2576\e[0m (\e[1;33m9590\e[0m)  \e[1;32m╶\e[0m  Light Right\n"
    printf "  \e[1;36mU+2577\e[0m (\e[1;33m9591\e[0m)  \e[1;32m╷\e[0m  Light Down\n"
    printf "  \e[1;36mU+2578\e[0m (\e[1;33m9592\e[0m)  \e[1;32m╸\e[0m  Heavy Left\n"
    printf "  \e[1;36mU+2579\e[0m (\e[1;33m9593\e[0m)  \e[1;32m╹\e[0m  Heavy Up\n"
    printf "  \e[1;36mU+257A\e[0m (\e[1;33m9594\e[0m)  \e[1;32m╺\e[0m  Heavy Right\n"
    printf "  \e[1;36mU+257B\e[0m (\e[1;33m9595\e[0m)  \e[1;32m╻\e[0m  Heavy Down\n"
    printf "  \e[1;36mU+257C\e[0m (\e[1;33m9596\e[0m)  \e[1;32m╼\e[0m  Light Left & Heavy Right\n"
    printf "  \e[1;36mU+257D\e[0m (\e[1;33m9597\e[0m)  \e[1;32m╽\e[0m  Light Up & Heavy Down\n"
    printf "  \e[1;36mU+257E\e[0m (\e[1;33m9598\e[0m)  \e[1;32m╾\e[0m  Heavy Left & Light Right\n"
    printf "  \e[1;36mU+257F\e[0m (\e[1;33m9599\e[0m)  \e[1;32m╿\e[0m  Heavy Up & Light Down\n"
    echo
}

if [ "$BOX_ONLY" = true ]; then
    run_box_drawing_tests
    echo "==========================================="
    echo "BOX DRAWING TEST COMPLETE"
    echo "==========================================="
    echo
    exit 0
fi


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

    grid_width=$((cols - 8))
    if [ $grid_width -lt 10 ]; then grid_width=10; fi

    # 1. Column Tens & Units Header Rulers
    hundreds_line="       "
    tens_line="Col:   "
    units_line="       "
    for ((c=1; c<=grid_width; c++)); do
        unit=$((c % 10))
        if [ $((c % 100)) -eq 0 ]; then
            hundreds_line="${hundreds_line}$((c / 100))"
        else
            hundreds_line="${hundreds_line} "
        fi
        if [ $((c % 10)) -eq 0 ]; then
            ten_digit=$(( (c / 10) % 10 ))
            tens_line="${tens_line}${ten_digit}"
        else
            tens_line="${tens_line} "
        fi
        units_line="${units_line}${unit}"
    done

    if [ $grid_width -ge 100 ]; then
        printf "\e[1;30m%s\e[0m\n" "$hundreds_line"
    fi
    printf "\e[1;33m%s\e[0m\n" "$tens_line"
    printf "\e[1;36m%s\e[0m\n" "$units_line"

    # 2. Top Box Line
    border_h=""
    for ((k=0; k<grid_width; k++)); do border_h="${border_h}─"; done

    printf "\e[1;35m┌─────┬%s┐\e[0m\n" "$border_h"

    letters="abcdefghijklmnopqrstuvwxyz"
    num_pattern=""
    alpha_pattern=""
    for ((c=1; c<=grid_width; c++)); do
        num_pattern="${num_pattern}$((c % 10))"
        char_idx=$(( (c - 1) % 26 ))
        alpha_pattern="${alpha_pattern}${letters:char_idx:1}"
    done

    max_rows=$((lines - 10))
    if [ $max_rows -lt 4 ]; then max_rows=4; fi

    for ((r=1; r<=max_rows; r++)); do
        row_tag=$(printf "R%02d" "$r")
        if [ $((r % 2)) -eq 1 ]; then
            row_content="$num_pattern"
        else
            row_content="$alpha_pattern"
        fi
        printf "\e[1;35m│\e[1;32m %s \e[1;35m│\e[0m%s\e[1;35m│\e[0m\n" "$row_tag" "$row_content"
    done

    printf "\e[1;35m└─────┴%s┘\e[0m\n" "$border_h"
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
                char_idx=$(( (c - 5) % 26 ))
                grid_fill="${grid_fill}${letters:char_idx:1}"
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
    printf -v mid_msg_str " VELOX FULL MATRIX GRID [ %d ROWS x %d COLS ] " "$lines" "$cols"
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
run_box_drawing_tests
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

grid_width=$((cols - 8))
if [ $grid_width -lt 10 ]; then grid_width=10; fi

# 1. Column Tens & Units Header Rulers
hundreds_line="       "
tens_line="Col:   "
units_line="       "
for ((c=1; c<=grid_width; c++)); do
    unit=$((c % 10))
    if [ $((c % 100)) -eq 0 ]; then
        hundreds_line="${hundreds_line}$((c / 100))"
    else
        hundreds_line="${hundreds_line} "
    fi
    if [ $((c % 10)) -eq 0 ]; then
        ten_digit=$(( (c / 10) % 10 ))
        tens_line="${tens_line}${ten_digit}"
    else
        tens_line="${tens_line} "
    fi
    units_line="${units_line}${unit}"
done

if [ $grid_width -ge 100 ]; then
    printf "\e[1;30m%s\e[0m\n" "$hundreds_line"
fi
printf "\e[1;33m%s\e[0m\n" "$tens_line"
printf "\e[1;36m%s\e[0m\n" "$units_line"

# 2. Top Box Line
border_h=""
for ((k=0; k<grid_width; k++)); do border_h="${border_h}─"; done

printf "\e[1;35m┌─────┬%s┐\e[0m\n" "$border_h"

# 3. Data Rows with Row Index & Column Unit Patterns
max_rows=8
if [ $((lines / 2)) -gt 8 ]; then max_rows=$((lines / 2)); fi

letters="abcdefghijklmnopqrstuvwxyz"
num_pattern=""
alpha_pattern=""
for ((c=1; c<=grid_width; c++)); do
    num_pattern="${num_pattern}$((c % 10))"
    char_idx=$(( (c - 1) % 26 ))
    alpha_pattern="${alpha_pattern}${letters:char_idx:1}"
done

for ((r=1; r<=max_rows; r++)); do
    row_tag=$(printf "R%02d" "$r")
    if [ $((r % 2)) -eq 1 ]; then
        row_content="$num_pattern"
    else
        row_content="$alpha_pattern"
    fi
    printf "\e[1;35m│\e[1;32m %s \e[1;35m│\e[0m%s\e[1;35m│\e[0m\n" "$row_tag" "$row_content"
done

printf "\e[1;35m└─────┴%s┘\e[0m\n" "$border_h"
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
                char_idx=$(( (c - 5) % 26 ))
                grid_fill="${grid_fill}${letters:char_idx:1}"
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
    printf -v mid_msg_str " VELOX FULL MATRIX GRID [ %d ROWS x %d COLS ] " "$lines" "$cols"
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
