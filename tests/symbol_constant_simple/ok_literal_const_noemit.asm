#ruledef test
{
    ld {x} => 0x55 @ x`8
}


#const(noemit) val = 0xaa
ld val ; = 0x55aa