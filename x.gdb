# this is a gdb script
# just for testing / reverse engineering
#
# run with:
# $ gdb -p (pidof ed8_3_PC.exe) --command=x.gdb

display $rax
display $rbx

handle SIGUSR1 pass nostop noprint

## EN
#rwatch *(unsigned char *) 0x1416c535d
#awatch *(unsigned char *) 0x1416c535d
#condition 1 ($rip != 0x00000001402fd952) && ($rip != 0x000000014032a5b1) && ($rip != 0x000000014032a5b8) && ($rip != 0x00000001402fcf77) && ($rip != 0x00000001402fd222) && ($rip != 0x00000001402fd22a) && ($rip != 0x00000001402fd3e6) && ($rip != 0x00000001403dfdaa) && ($rip != 0x00000001402fc96e)

#watch *(unsigned char *) 0x1416c535d
#condition 1 ((*(unsigned char *) 0x1416c535d) == 0)

#watch *0x14032a5b1 if $rbx != 0x1416c1620
#watch *0x1403dfda3 if $rax != 0x1416c1620

## JP

# cutscene skip flag
set *(unsigned char *) 0x1416af22d = 1

# cutscene skip flag setters: set to 1 instead of 0
set *(unsigned char *) 0x1403210a7 = 1
set *(unsigned char *) (0x14030acda + 8) = 1
set *(unsigned char *) 0x1403d5949 = 1

# watch cutscene skip flag changes
watch *(unsigned char *) 0x1416af22d
# except from "known" locations
condition 1 ($rip != 0x1402f5a36) && ($rip != 0x1403210a8) && ($rip != 0x14030ace5) && ($rip != 0x1403d594a)

# watch cutscene skip flag setters
#watch *0x1403210a1 if $rbx != 0x1416ab4f0
#watch *0x14030acda if $rbx != 0x1416ab4f0
#watch *0x1403d5943 if $rax != 0x1416ab4f0


continue
