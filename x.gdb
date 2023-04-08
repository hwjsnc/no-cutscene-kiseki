# this is a gdb script
# just for testing / reverse engineering
#
# run with:
# $ gdb -p (pidof ed8_3_PC.exe) --command=x.gdb

display $rax
display $rbx

handle SIGUSR1 pass nostop noprint
#rwatch *(unsigned char *) 0x1416c535d
#awatch *(unsigned char *) 0x1416c535d
#condition 1 ($rip != 0x00000001402fd952) && ($rip != 0x000000014032a5b1) && ($rip != 0x000000014032a5b8) && ($rip != 0x00000001402fcf77) && ($rip != 0x00000001402fd222) && ($rip != 0x00000001402fd22a) && ($rip != 0x00000001402fd3e6) && ($rip != 0x00000001403dfdaa) && ($rip != 0x00000001402fc96e)

#watch *(unsigned char *) 0x1416c535d
#condition 1 ((*(unsigned char *) 0x1416c535d) == 0)

watch *0x14032a5b1 if $rbx != 0x1416c1620
#watch *0x1403dfda3 if $rax != 0x1416c1620
continue
