#ruledef test
{
    ld {x} =>
    {
        assert("your custom message!", x < 0x10)
        0x55 @ x`8
    }
}

ld 0x15 ; error: failed / note:_:3: within / error:_:5: expected boolean