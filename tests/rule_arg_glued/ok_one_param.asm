#ruledef test
{
    ld r{x} => 0x55 @ x`8
}

ld r0 ; = 0x5500
ld r(0) ; = 0x5500
ld r 0 ; = 0x5500
ld r12 ; = 0x550c
ld r(6 + 6) ; = 0x550c
ld r 6 + 6 ; = 0x550c
ld r257 ; = 0x5501
ld r 0xff ; = 0x55ff