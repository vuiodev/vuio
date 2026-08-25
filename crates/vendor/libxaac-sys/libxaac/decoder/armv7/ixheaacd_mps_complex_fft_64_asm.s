.text
.p2align 2
.global ixheaacd_mps_complex_fft_64_asm
.type ixheaacd_mps_complex_fft_64_asm, %function

ixheaacd_mps_complex_fft_64_asm:
    @LDR    r4,[sp]
    STMFD           sp!, {r0-r12, lr}
    LDR             r4, [sp, #0x38]
    SUB             sp, sp, #0x44
    LDR             r0, [sp, #0x48]
    EOR             r0, r0, r0, ASR #31
    CLZ             r0, r0
    SUB             r12, r0, #16        @dig_rev_shift = norm32(npoints) + 1 -16@
    SUB             r0, r0, #1
    RSB             r0, r0, #0x1e
    AND             r1, r0, #1
    STR             r1, [sp, #0x30]
    MOV             r1, r0, ASR #1
    LDR             r0, [sp, #0x48]     @npoints
    STR             r1, [sp, #0x18]
    MOV             lr, r0, LSL #1      @(npoints >>1) * 4
    MOV             r0, #0
    MOV             r12, r4
FIRST_STAGE_R4:
    LDRB            r10, [r12, r0, LSR #2]


    ADD             r1, r2, r10, LSL #2
    LDRD            r4, [r1]            @r4=x0r,  r5=x0i
    ADD             r1, r1, lr
    LDRD            r8, [r1]            @r8=x1r,  r9=x1i
    ADD             r1, r1, lr
    LDRD            r6, [r1]            @r6=x2r,  r7=x2i
    ADD             r1, r1, lr
    LDRD            r10, [r1]           @r10=x3r, r11=x3i
    ADD             r0, r0, #4
    CMP             r0, lr, ASR #1

    ADD             r4, r4, r6          @x0r = x0r + x2r@
    ADD             r5, r5, r7          @x0i = x0i + x2i@
    SUB             r6, r4, r6, lsl#1   @x2r = x0r - (x2r << 1)@
    SUB             r7, r5, r7, lsl#1   @x2i = x0i - (x2i << 1)@
    ADD             r8, r8, r10         @x1r = x1r + x3r@
    ADD             r9, r9, r11         @x1i = x1i + x3i@
    SUB             r1, r8, r10, lsl#1  @x3r = x1r - (x3r << 1)@
    SUB             r11, r9, r11, lsl#1 @x3i = x1i - (x3i << 1)@

    ADD             r4, r4, r8          @x0r = x0r + x1r@
    ADD             r5, r5, r9          @x0i = x0i + x1i@
    SUB             r8, r4, r8, lsl#1   @x1r = x0r - (x1r << 1)@
    SUB             r9, r5, r9, lsl#1   @x1i = x0i - (x1i << 1)
    ADD             r6, r6, r11         @x2r = x2r + x3i@
    SUB             r7, r7, r1          @x2i = x2i - x3r@
    SUB             r10, r6, r11, lsl#1 @x3i = x2r - (x3i << 1)@
    ADD             r11, r7, r1, lsl#1  @x3r = x2i + (x3r << 1)@

    STMIA           r3!, {r4-r11}
    BLT             FIRST_STAGE_R4
    LDR             r1, [sp, #0x18]
    LDR             r0, [sp, #0x48]
    MOV             r12, #0x40          @nodespacing = 64@
    STR             r12, [sp, #0x38]
    LDR             r12, [sp, #0x48]
    SUB             r3, r3, r0, LSL #3
    SUBS            r1, r1, #1
    STR             r3, [sp, #0x50]
    MOV             r4, r12, ASR #4
    MOV             r0, #4
    STR             r4, [sp, #0x34]
    STR             r1, [sp, #0x3c]
    BLE             EXIT
OUTER_LOOP:
    LDR             r1, [sp, #0x44]
    LDR             r12, [sp, #0x50]    @WORD32 *data = ptr_y@
    STR             r1, [sp, #0x2c]
    LDR             r1, [sp, #0x34]

    MOV             r0, r0, LSL #3      @(del<<1) * 4
LOOP_TRIVIAL_TWIDDLE:
    LDRD            r4, [r12]           @r4=x0r,  r5=x0i
    ADD             r12, r12, r0
    LDRD            r6, [r12]           @r6=x1r,  r7=x1i
    ADD             r12, r12, r0
    LDRD            r8, [r12]           @r8=x2r,  r9=x2i
    ADD             r12, r12, r0
    LDRD            r10, [r12]          @r10=x3r, r11=x3i

@MOV    r4,r4,ASR #1
@MOV    r5,r5,ASR #1
@MOV    r6,r6,ASR #1
@MOV    r7,r7,ASR #1
@MOV    r8,r8,ASR #1
@MOV    r9,r9,ASR #1
@MOV    r10,r10,ASR #1
@MOV    r11,r11,ASR #1

    ADD             r4, r4, r8          @x0r = x0r + x2r@
    ADD             r5, r5, r9          @x0i = x0i + x2i@
    SUB             r8, r4, r8, lsl #1  @x2r = x0r - (x2r << 1)@
    SUB             r9, r5, r9, lsl #1  @x2i = x0i - (x2i << 1)@
    ADD             r6, r6, r10         @x1r = x1r + x3r@
    ADD             r7, r7, r11         @x1i = x1i + x3i@
    SUB             r2, r6, r10, lsl #1 @x3r = x1r - (x3r << 1)@
    SUB             r11, r7, r11, lsl #1 @x3i = x1i - (x3i << 1)@

    ADD             r4, r4, r6          @x0r = x0r + x1r@
    ADD             r5, r5, r7          @x0i = x0i + x1i@
@MOV    r4,r4,ASR #1
@MOV    r5,r5,ASR #1
    SUB             r6, r4, r6, lsl #1  @x1r = x0r - (x1r << 1)@
    SUB             r7, r5, r7, lsl #1  @x1i = x0i - (x1i << 1)
    ADD             r8, r8, r11         @x2r = x2r + x3i@
    SUB             r9, r9, r2          @x2i = x2i - x3r@
    SUB             r10, r8, r11, lsl#1 @x3i = x2r - (x3i << 1)@
    ADD             r11, r9, r2, lsl#1  @x3r = x2i + (x3r << 1)

    STRD            r10, [r12]          @r10=x3r, r11=x3i
    SUB             r12, r12, r0
    STRD            r6, [r12]           @r6=x1r,  r7=x1i
    SUB             r12, r12, r0
    STRD            r8, [r12]           @r8=x2r,  r9=x2i
    SUB             r12, r12, r0
    STRD            r4, [r12]           @r4=x0r,  r5=x0i
    ADD             r12, r12, r0, lsl #2

    SUBS            r1, r1, #1
    BNE             LOOP_TRIVIAL_TWIDDLE

    MOV             r0, r0, ASR #3
    LDR             r4, [sp, #0x38]
    LDR             r3, [sp, #0x50]
    MUL             r1, r0, r4
    ADD             r12, r3, #8
    STR             r1, [sp, #0x40]
    MOV             r3, r1, ASR #2
    ADD             r3, r3, r1, ASR #3
    SUB             r3, r3, r1, ASR #4
    ADD             r3, r3, r1, ASR #5
    SUB             r3, r3, r1, ASR #6
    ADD             r3, r3, r1, ASR #7
    SUB             r3, r3, r1, ASR #8
    STR             r3, [sp, #0x18]
SECOND_LOOP:
    LDR             r3, [sp, #0x2c]
    LDR             r14, [sp, #0x34]
    MOV             r0, r0, LSL #3      @(del<<1) * 4
    LDR             r1, [r3, r4, LSL #3]! @ w1h = *(twiddles + 2*j)@
    LDR             r2, [r3, #0x04]     @w1l = *(twiddles + 2*j + 1)@
    LDR             r5, [r3, r4, LSL #3]! @w2h = *(twiddles + 2*(j<<1))@
    LDR             r6, [r3, #0x04]     @w2l = *(twiddles + 2*(j<<1) + 1)@
    LDR             r7, [r3, r4, LSL #3]! @w3h = *(twiddles + 2*j + 2*(j<<1))@
    LDR             r8, [r3, #0x04]     @w3l = *(twiddles + 2*j + 2*(j<<1) + 1)@

    STR             r4, [sp, #0x24]
    STR             r1, [sp, #0x14]
    STR             r2, [sp, #0x10]
    STR             r5, [sp, #0x0c]
    STR             r6, [sp, #0x08]
    STR             r7, [sp, #0x04]
    STR             r8, [sp]

RADIX4_BFLY:

    LDRD            r6, [r12, r0]!      @r6=x1r,  r7=x1i
    LDRD            r8, [r12, r0]!      @r8=x2r,  r9=x2i
    LDRD            r10, [r12, r0]      @r10=x3r, r11=x3i
    SUBS            r14, r14, #1

    LDR             r1, [sp, #0x14]
    LDR             r2, [sp, #0x10]

    SMULL           r3, r4, r6, r2      @ixheaac_mult32(x1r,w1l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r6, r6, r1      @mult32x16hin32(x1r,W1h)
    LSR             r3, r3, #31
    ORR             r6, r3, r6, LSL#1
    SMULL           r3, r5, r7, r1      @mult32x16hin32(x1i,W1h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r7, r7, r2      @ixheaac_mac32(ixheaac_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r7, r3, r7, LSL#1
    ADD             r7, r7, r6
    SUB             r6, r4, r5          @

    LDR             r1, [sp, #0x0c]
    LDR             r2, [sp, #0x08]

    SMULL           r3, r4, r8, r2      @ixheaac_mult32(x2r,w2l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r8, r8, r1      @mult32x16hin32(x2r,W2h)
    LSR             r3, r3, #31
    ORR             r8, r3, r8, LSL#1
    SMULL           r3, r5, r9, r1      @mult32x16hin32(x2i,W2h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r9, r9, r2      @ixheaac_mac32(ixheacd_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r9, r3, r9, LSL#1
    ADD             r9, r9, r8
    SUB             r8, r4, r5          @

    LDR             r1, [sp, #0x04]
    LDR             r2, [sp]

    SMULL           r3, r4, r10, r2     @ixheaac_mult32(x3r,w3l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r10, r10, r1    @mult32x16hin32(x3r,W3h)
    LSR             r3, r3, #31
    ORR             r10, r3, r10, LSL#1
    SMULL           r3, r5, r11, r1     @mult32x16hin32(x3i,W3h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r11, r11, r2    @ixheaac_mac32(ixheacd_mult32(x3r,w3h) ,x3i,w3l)
    LSR             r3, r3, #31
    ORR             r11, r3, r11, LSL#1
    ADD             r11, r11, r10
    SUB             r10, r4, r5         @

    @SUB   r12,r12,r0,lsl #1
    @LDRD     r4,[r12]      @r4=x0r,  r5=x0i
    LDR             r4, [r12, -r0, lsl #1]! @
    LDR             r5, [r12, #0x04]


    ADD             r4, r8, r4          @x0r = x0r + x2r@
    ADD             r5, r9, r5          @x0i = x0i + x2i@
    SUB             r8, r4, r8, lsl#1   @x2r = x0r - (x2r << 1)@
    SUB             r9, r5, r9, lsl#1   @x2i = x0i - (x2i << 1)@
    ADD             r6, r6, r10         @x1r = x1r + x3r@
    ADD             r7, r7, r11         @x1i = x1i + x3i@
    SUB             r10, r6, r10, lsl#1 @x3r = x1r - (x3r << 1)@
    SUB             r11, r7, r11, lsl#1 @x3i = x1i - (x3i << 1)@

    ADD             r4, r4, r6          @x0r = x0r + x1r@
    ADD             r5, r5, r7          @x0i = x0i + x1i@
    SUB             r6, r4, r6, lsl#1   @x1r = x0r - (x1r << 1)@
    SUB             r7, r5, r7, lsl#1   @x1i = x0i - (x1i << 1)
    STRD            r4, [r12]           @r4=x0r,  r5=x0i
    ADD             r12, r12, r0

    ADD             r8, r8, r11         @x2r = x2r + x3i@
    SUB             r9, r9, r10         @x2i = x2i - x3r@
    SUB             r4, r8, r11, lsl#1  @x3i = x2r - (x3i << 1)@
    ADD             r5, r9, r10, lsl#1  @x3r = x2i + (x3r << 1)

    STRD            r8, [r12]           @r8=x2r,  r9=x2i
    ADD             r12, r12, r0
    STRD            r6, [r12]           @r6=x1r,  r7=x1i
    ADD             r12, r12, r0
    STRD            r4, [r12]           @r10=x3r, r11=x3i
    ADD             r12, r12, r0

    BNE             RADIX4_BFLY
    MOV             r0, r0, ASR #3

    LDR             r1, [sp, #0x48]
    LDR             r4, [sp, #0x24]
    SUB             r1, r12, r1, LSL #3
    LDR             r6, [sp, #0x38]
    ADD             r12, r1, #8
    LDR             r7, [sp, #0x18]
    ADD             r4, r4, r6
    CMP             r4, r7
    BLE             SECOND_LOOP

SECOND_LOOP_2:
    LDR             r3, [sp, #0x2c]
    LDR             r14, [sp, #0x34]
    MOV             r0, r0, LSL #3      @(del<<1) * 4

    LDR             r1, [r3, r4, LSL #3]! @ w1h = *(twiddles + 2*j)@
    LDR             r2, [r3, #0x04]     @w1l = *(twiddles + 2*j + 1)@
    LDR             r5, [r3, r4, LSL #3]! @w2h = *(twiddles + 2*(j<<1))@
    LDR             r6, [r3, #0x04]     @w2l = *(twiddles + 2*(j<<1) + 1)@
    SUB             r3, r3, #2048       @ 512 *4
    LDR             r7, [r3, r4, LSL #3]! @w3h = *(twiddles + 2*j + 2*(j<<1))@
    LDR             r8, [r3, #0x04]     @w3l = *(twiddles + 2*j + 2*(j<<1) + 1)@

    STR             r4, [sp, #0x24]

    STR             r1, [sp, #0x14]
    STR             r2, [sp, #0x10]
    STR             r5, [sp, #0x0c]
    STR             r6, [sp, #0x08]
    STR             r7, [sp, #0x04]
    STR             r8, [sp]

RADIX4_BFLY_2:
    LDRD            r6, [r12, r0]!      @r6=x1r,  r7=x1i
    LDRD            r8, [r12, r0]!      @r8=x2r,  r9=x2i
    LDRD            r10, [r12, r0]      @r10=x3r, r11=x3i
    SUBS            r14, r14, #1
    LDR             r1, [sp, #0x14]
    LDR             r2, [sp, #0x10]

    SMULL           r3, r4, r6, r2      @ixheaac_mult32(x1r,w1l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r6, r6, r1      @mult32x16hin32(x1r,W1h)
    LSR             r3, r3, #31
    ORR             r6, r3, r6, LSL#1
    SMULL           r3, r5, r7, r1      @mult32x16hin32(x1i,W1h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r7, r7, r2      @ixheaac_mac32(ixheaac_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r7, r3, r7, LSL#1
    ADD             r7, r7, r6
    SUB             r6, r4, r5          @

    LDR             r1, [sp, #0x0c]
    LDR             r2, [sp, #0x08]

    SMULL           r3, r4, r8, r2      @ixheaac_mult32(x2r,w2l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r8, r8, r1      @mult32x16hin32(x2r,W2h)
    LSR             r3, r3, #31
    ORR             r8, r3, r8, LSL#1
    SMULL           r3, r5, r9, r1      @mult32x16hin32(x2i,W2h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r9, r9, r2      @ixheaac_mac32(ixheacd_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r9, r3, r9, LSL#1
    ADD             r9, r9, r8
    SUB             r8, r4, r5          @

    LDR             r1, [sp, #0x04]
    LDR             r2, [sp]

    SMULL           r3, r4, r10, r2     @ixheaac_mult32(x3r,w3l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r10, r10, r1    @mult32x16hin32(x3r,W3h)
    LSR             r3, r3, #31
    ORR             r10, r3, r10, LSL#1
    SMULL           r3, r5, r11, r1     @mult32x16hin32(x3i,W3h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r11, r11, r2    @ixheaac_mac32(ixheacd_mult32(x3r,w3h) ,x3i,w3l)
    LSR             r3, r3, #31
    ORR             r11, r3, r11, LSL#1
    ADD             r10, r11, r10
    SUB             r11, r5, r4         @

    @SUB    r12,r12,r0,lsl #1
    @LDRD     r4,[r12]      @r4=x0r,  r5=x0i
    LDR             r4, [r12, -r0, lsl #1]! @
    LDR             r5, [r12, #0x04]


    ADD             r4, r8, r4          @x0r = x0r + x2r@
    ADD             r5, r9, r5          @x0i = x0i + x2i@
    SUB             r8, r4, r8, lsl#1   @x2r = x0r - (x2r << 1)@
    SUB             r9, r5, r9, lsl#1   @x2i = x0i - (x2i << 1)@
    ADD             r6, r6, r10         @x1r = x1r + x3r@
    ADD             r7, r7, r11         @x1i = x1i + x3i@
    SUB             r10, r6, r10, lsl#1 @x3r = x1r - (x3r << 1)@
    SUB             r11, r7, r11, lsl#1 @x3i = x1i - (x3i << 1)@

    ADD             r4, r4, r6          @x0r = x0r + x1r@
    ADD             r5, r5, r7          @x0i = x0i + x1i@
    SUB             r6, r4, r6, lsl#1   @x1r = x0r - (x1r << 1)@
    SUB             r7, r5, r7, lsl#1   @x1i = x0i - (x1i << 1)
    STRD            r4, [r12]           @r4=x0r,  r5=x0i
    ADD             r12, r12, r0

    ADD             r8, r8, r11         @x2r = x2r + x3i@
    SUB             r9, r9, r10         @x2i = x2i - x3r@
    SUB             r4, r8, r11, lsl#1  @x3i = x2r - (x3i << 1)@
    ADD             r5, r9, r10, lsl#1  @x3r = x2i + (x3r << 1)

    STRD            r8, [r12]           @r8=x2r,  r9=x2i
    ADD             r12, r12, r0
    STRD            r6, [r12]           @r6=x1r,  r7=x1i
    ADD             r12, r12, r0
    STRD            r4, [r12]           @r10=x3r, r11=x3i
    ADD             r12, r12, r0

    BNE             RADIX4_BFLY_2
    MOV             r0, r0, ASR #3

    LDR             r1, [sp, #0x48]
    LDR             r4, [sp, #0x24]
    SUB             r1, r12, r1, LSL #3
    LDR             r6, [sp, #0x38]
    ADD             r12, r1, #8
    LDR             r7, [sp, #0x40]
    ADD             r4, r4, r6
    CMP             r4, r7, ASR #1
    BLE             SECOND_LOOP_2
    LDR             r7, [sp, #0x18]
    CMP             r4, r7, LSL #1
    BGT             SECOND_LOOP_4

SECOND_LOOP_3:
    LDR             r3, [sp, #0x2c]
    LDR             r14, [sp, #0x34]
    MOV             r0, r0, LSL #3      @(del<<1) * 4

    LDR             r1, [r3, r4, LSL #3]! @ w1h = *(twiddles + 2*j)@
    LDR             r2, [r3, #0x04]     @w1l = *(twiddles + 2*j + 1)@
    SUB             r3, r3, #2048       @ 512 *4
    LDR             r5, [r3, r4, LSL #3]! @w2h = *(twiddles + 2*(j<<1))@
    LDR             r6, [r3, #0x04]     @w2l = *(twiddles + 2*(j<<1) + 1)@
    LDR             r7, [r3, r4, LSL #3]! @w3h = *(twiddles + 2*j + 2*(j<<1))@
    LDR             r8, [r3, #0x04]     @w3l = *(twiddles + 2*j + 2*(j<<1) + 1)@

    STR             r4, [sp, #0x24]
    STR             r1, [sp, #0x14]
    STR             r2, [sp, #0x10]
    STR             r5, [sp, #0x0c]
    STR             r6, [sp, #0x08]
    STR             r7, [sp, #0x04]
    STR             r8, [sp]


RADIX4_BFLY_3:
    LDRD            r6, [r12, r0]!      @r6=x1r,  r7=x1i
    LDRD            r8, [r12, r0]!      @r8=x2r,  r9=x2i
    LDRD            r10, [r12, r0]      @r10=x3r, r11=x3i
    SUBS            r14, r14, #1

    LDR             r1, [sp, #0x14]
    LDR             r2, [sp, #0x10]

    SMULL           r3, r4, r6, r2      @ixheaac_mult32(x1r,w1l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r6, r6, r1      @mult32x16hin32(x1r,W1h)
    LSR             r3, r3, #31
    ORR             r6, r3, r6, LSL#1
    SMULL           r3, r5, r7, r1      @mult32x16hin32(x1i,W1h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r7, r7, r2      @ixheaac_mac32(ixheaac_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r7, r3, r7, LSL#1
    ADD             r7, r7, r6
    SUB             r6, r4, r5          @

    LDR             r1, [sp, #0x0c]
    LDR             r2, [sp, #0x08]

    SMULL           r3, r4, r8, r2      @ixheaac_mult32(x2r,w2l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r8, r8, r1      @mult32x16hin32(x2r,W2h)
    LSR             r3, r3, #31
    ORR             r8, r3, r8, LSL#1
    SMULL           r3, r5, r9, r1      @mult32x16hin32(x2i,W2h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r9, r9, r2      @ixheaac_mac32(ixheacd_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r9, r3, r9, LSL#1
    ADD             r8, r9, r8
    SUB             r9, r5, r4          @

    LDR             r1, [sp, #0x04]
    LDR             r2, [sp]

    SMULL           r3, r4, r10, r2     @ixheaac_mult32(x3r,w3l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r10, r10, r1    @mult32x16hin32(x3r,W3h)
    LSR             r3, r3, #31
    ORR             r10, r3, r10, LSL#1
    SMULL           r3, r5, r11, r1     @mult32x16hin32(x3i,W3h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r11, r11, r2    @ixheaac_mac32(ixheacd_mult32(x3r,w3h) ,x3i,w3l)
    LSR             r3, r3, #31
    ORR             r11, r3, r11, LSL#1
    ADD             r10, r11, r10
    SUB             r11, r5, r4         @

    @SUB    r12,r12,r0,lsl #1
    @LDRD     r4,[r12]      @r4=x0r,  r5=x0i
    LDR             r4, [r12, -r0, lsl #1]! @
    LDR             r5, [r12, #0x04]


    ADD             r4, r8, r4          @x0r = x0r + x2r@
    ADD             r5, r9, r5          @x0i = x0i + x2i@
    SUB             r8, r4, r8, lsl#1   @x2r = x0r - (x2r << 1)@
    SUB             r9, r5, r9, lsl#1   @x2i = x0i - (x2i << 1)@
    ADD             r6, r6, r10         @x1r = x1r + x3r@
    ADD             r7, r7, r11         @x1i = x1i + x3i@
    SUB             r10, r6, r10, lsl#1 @x3r = x1r - (x3r << 1)@
    SUB             r11, r7, r11, lsl#1 @x3i = x1i - (x3i << 1)@

    ADD             r4, r4, r6          @x0r = x0r + x1r@
    ADD             r5, r5, r7          @x0i = x0i + x1i@
    SUB             r6, r4, r6, lsl#1   @x1r = x0r - (x1r << 1)@
    SUB             r7, r5, r7, lsl#1   @x1i = x0i - (x1i << 1)
    STRD            r4, [r12]           @r4=x0r,  r5=x0i
    ADD             r12, r12, r0

    ADD             r8, r8, r11         @x2r = x2r + x3i@
    SUB             r9, r9, r10         @x2i = x2i - x3r@
    SUB             r4, r8, r11, lsl#1  @x3i = x2r - (x3i << 1)@
    ADD             r5, r9, r10, lsl#1  @x3r = x2i + (x3r << 1)

    STRD            r8, [r12]           @r8=x2r,  r9=x2i
    ADD             r12, r12, r0
    STRD            r6, [r12]           @r6=x1r,  r7=x1i
    ADD             r12, r12, r0
    STRD            r4, [r12]           @r10=x3r, r11=x3i
    ADD             r12, r12, r0

    BNE             RADIX4_BFLY_3
    MOV             r0, r0, ASR #3

    LDR             r1, [sp, #0x48]
    LDR             r4, [sp, #0x24]
    SUB             r1, r12, r1, LSL #3
    LDR             r6, [sp, #0x38]
    ADD             r12, r1, #8
    LDR             r7, [sp, #0x18]
    ADD             r4, r4, r6
    CMP             r4, r7, LSL #1
    BLE             SECOND_LOOP_3

SECOND_LOOP_4:
    LDR             r3, [sp, #0x2c]
    LDR             r14, [sp, #0x34]
    MOV             r0, r0, LSL #3      @(del<<1) * 4

    LDR             r1, [r3, r4, LSL #3]! @ w1h = *(twiddles + 2*j)@
    LDR             r2, [r3, #0x04]     @w1l = *(twiddles + 2*j + 1)@
    SUB             r3, r3, #2048       @ 512 *4
    LDR             r5, [r3, r4, LSL #3]! @w2h = *(twiddles + 2*(j<<1))@
    LDR             r6, [r3, #0x04]     @w2l = *(twiddles + 2*(j<<1) + 1)@
    SUB             r3, r3, #2048       @ 512 *4
    LDR             r7, [r3, r4, LSL #3]! @w3h = *(twiddles + 2*j + 2*(j<<1))@
    LDR             r8, [r3, #0x04]     @w3l = *(twiddles + 2*j + 2*(j<<1) + 1)@


    STR             r4, [sp, #0x24]
    STR             r1, [sp, #0x14]
    STR             r2, [sp, #0x10]
    STR             r5, [sp, #0x0c]
    STR             r6, [sp, #0x08]
    STR             r7, [sp, #0x04]
    STR             r8, [sp]

RADIX4_BFLY_4:
    LDRD            r6, [r12, r0]!      @r6=x1r,  r7=x1i
    LDRD            r8, [r12, r0]!      @r8=x2r,  r9=x2i
    LDRD            r10, [r12, r0]      @r10=x3r, r11=x3i
    SUBS            r14, r14, #1

    LDR             r1, [sp, #0x14]
    LDR             r2, [sp, #0x10]

    SMULL           r3, r4, r6, r2      @ixheaac_mult32(x1r,w1l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r6, r6, r1      @mult32x16hin32(x1r,W1h)
    LSR             r3, r3, #31
    ORR             r6, r3, r6, LSL#1
    SMULL           r3, r5, r7, r1      @mult32x16hin32(x1i,W1h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r7, r7, r2      @ixheaac_mac32(ixheaac_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r7, r3, r7, LSL#1
    ADD             r7, r7, r6
    SUB             r6, r4, r5          @

    LDR             r1, [sp, #0x0c]
    LDR             r2, [sp, #0x08]

    SMULL           r3, r4, r8, r2      @ixheaac_mult32(x2r,w2l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r8, r8, r1      @mult32x16hin32(x2r,W2h)
    LSR             r3, r3, #31
    ORR             r8, r3, r8, LSL#1
    SMULL           r3, r5, r9, r1      @mult32x16hin32(x2i,W2h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r9, r9, r2      @ixheaac_mac32(ixheacd_mult32(x1r,w1h) ,x1i,w1l)
    LSR             r3, r3, #31
    ORR             r9, r3, r9, LSL#1
    ADD             r8, r9, r8
    SUB             r9, r5, r4          @

    LDR             r1, [sp, #0x04]
    LDR             r2, [sp]

    SMULL           r3, r4, r10, r2     @ixheaac_mult32(x3r,w3l)
    LSR             r3, r3, #31
    ORR             r4, r3, r4, LSL#1
    SMULL           r3, r10, r10, r1    @mult32x16hin32(x3r,W3h)
    LSR             r3, r3, #31
    ORR             r10, r3, r10, LSL#1
    SMULL           r3, r5, r11, r1     @mult32x16hin32(x3i,W3h)
    LSR             r3, r3, #31
    ORR             r5, r3, r5, LSL#1
    SMULL           r3, r11, r11, r2    @ixheaac_mac32(ixheacd_mult32(x3r,w3h) ,x3i,w3l)
    LSR             r3, r3, #31
    ORR             r11, r3, r11, LSL#1
    ADD             r11, r11, r10
    SUB             r10, r5, r4         @

    @SUB    r12,r12,r0,lsl #1
    @LDRD     r4,[r12]      @r4=x0r,  r5=x0i
    LDR             r4, [r12, -r0, lsl #1]! @
    LDR             r5, [r12, #0x04]


    ADD             r4, r8, r4          @x0r = x0r + x2r@
    ADD             r5, r9, r5          @x0i = x0i + x2i@
    SUB             r8, r4, r8, lsl#1   @x2r = x0r - (x2r << 1)@
    SUB             r9, r5, r9, lsl#1   @x2i = x0i - (x2i << 1)@
    ADD             r6, r6, r10         @x1r = x1r + x3r@
    SUB             r7, r7, r11         @x1i = x1i - x3i@
    SUB             r10, r6, r10, lsl#1 @x3r = x1r - (x3r << 1)@
    ADD             r11, r7, r11, lsl#1 @x3i = x1i + (x3i << 1)@

    ADD             r4, r4, r6          @x0r = x0r + x1r@
    ADD             r5, r5, r7          @x0i = x0i + x1i@
    SUB             r6, r4, r6, lsl#1   @x1r = x0r - (x1r << 1)@
    SUB             r7, r5, r7, lsl#1   @x1i = x0i - (x1i << 1)
    STRD            r4, [r12]           @r4=x0r,  r5=x0i
    ADD             r12, r12, r0
    ADD             r8, r8, r11         @x2r = x2r + x3i@
    SUB             r9, r9, r10         @x2i = x2i - x3r@
    SUB             r4, r8, r11, lsl#1  @x3i = x2r - (x3i << 1)@
    ADD             r5, r9, r10, lsl#1  @x3r = x2i + (x3r << 1)

    STRD            r8, [r12]           @r8=x2r,  r9=x2i
    ADD             r12, r12, r0
    STRD            r6, [r12]           @r6=x1r,  r7=x1i
    ADD             r12, r12, r0
    STRD            r4, [r12]           @r10=x3r, r11=x3i
    ADD             r12, r12, r0

    BNE             RADIX4_BFLY_4
    MOV             r0, r0, ASR #3

    LDR             r1, [sp, #0x48]
    LDR             r4, [sp, #0x24]
    SUB             r1, r12, r1, LSL #3
    LDR             r6, [sp, #0x38]
    ADD             r12, r1, #8
    LDR             r7, [sp, #0x40]
    ADD             r4, r4, r6
    CMP             r4, r7
    BLT             SECOND_LOOP_4

    LDR             r1, [sp, #0x38]
    MOV             r0, r0, LSL #2
    MOV             r1, r1, ASR #2
    STR             r1, [sp, #0x38]
    LDR             r1, [sp, #0x34]
    MOV             r1, r1, ASR #2
    STR             r1, [sp, #0x34]
    LDR             r1, [sp, #0x3c]
    SUBS            r1, r1, #1
    STR             r1, [sp, #0x3c]
    BGT             OUTER_LOOP

    LDR             r1, [sp, #0x30]
    CMP             r1, #0
    BEQ             EXIT
    LDR             r12, [sp, #0x38]
    LDR             r1, [sp, #0x44]
    CMP             r12, #0
    MOVEQ           r4, #1
    MOVNE           r4, r12, LSL #1
    MOVS            r3, r0
    BEQ             EXIT

    MOV             r3, r3, ASR #1
    LDR             r5, [sp, #0x50]
    MOV             r0, r0, LSL #3      @(del<<1) * 4
    STR             r1, [sp, #0x18]

EXIT:
    ADD             sp, sp, #0x54
    LDMFD           sp!, {r4-r12, pc}

