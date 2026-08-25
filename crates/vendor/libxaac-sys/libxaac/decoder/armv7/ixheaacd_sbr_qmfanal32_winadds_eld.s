.text
.p2align 2
.global ixheaacd_sbr_qmfanal32_winadds_eld
.type ixheaacd_sbr_qmfanal32_winadds_eld, %function

ixheaacd_sbr_qmfanal32_winadds_eld:

    STMFD           sp!, {R4-R12, R14}
    LDR             R5, [SP, #44]       @filterStates
    LDR             R6, [SP, #48]       @timeIn
    LDR             R7, [SP, #52]       @stride

    MOV             R9, R7, LSL #1

    ADD             r5, r5, #64
    MOV             r10, #3

LOOP:
    LDRSH           r4  , [R6], r9
    LDRSH           r8  , [R6], r9
    LDRSH           r11  , [R6], r9
    LDRSH           r12 , [R6], r9


    STRH            r4  , [r5 , #-2]!
    STRH            r8  , [r5 , #-2]!
    STRH            r11  , [r5 , #-2]!
    STRH            r12 , [r5 , #-2]!

    LDRSH           r4  , [R6], r9
    LDRSH           r8  , [R6], r9
    LDRSH           r11  , [R6], r9
    LDRSH           r12 , [R6], r9


    STRH            r4  , [r5 , #-2]!
    STRH            r8  , [r5 , #-2]!
    STRH            r11  , [r5 , #-2]!
    STRH            r12 , [r5 , #-2]!


    SUBS            r10, r10, #1

    BPL             LOOP

    LDR             R4, [SP, #40]      @winAdd

    MOV             R5, #8
    VLD1.16         D0, [R0]!           @tmpQ1[n +   0] load and incremented R0 by 8

    MOV             R6, #64
    MOV             R6, R6, LSL #1      @
    VLD1.16         {D1, D2}, [R2]!     @ tmpQmf_c1[2*(n +   0)] load and incremented

    MOV             R7, #244            @ NOT USED further

    MOV             R9, R0
    ADD             R0, R0, #120        @ incrementing R0 by 120 + 8 = 128

    MOV             R11, R4             @ Mov winAdd to R11
    VLD1.16         D2, [R0], R6        @ tmpQ1[n +  64] load and incremented by R6
    ADD             R11, R11, #128      @ increment winAdd by 128


    MOV             R10, R2             @
    ADD             R2, R2, #112        @ This should be 240 --> 112

    VMULL.S16       Q15, D0, D1
    VLD1.16         {D3, D4}, [R2]!     @ tmpQmf_c1[2*(n +  64)] load and incremented
    ADD             R2, R2, #112        @ This should be 112


    VLD1.16         D4, [R0], R6        @ tmpQ1[n + 128] load and incremented by R6
    VMLAL.S16       Q15, D2, D3

    VLD1.16         {D5, D6}, [R2]!     @ tmpQmf_c1[2*(n + 128)] load and incremented
    SUB             R10, R10, #8


    ADD             R2, R2, #112        @ This should be 112
    VLD1.16         D6, [R0], R6        @ tmpQ1[n + 192] load and incremented by R6
    VMLAL.S16       Q15, D4, D5

    VLD1.16         {D7, D8}, [R2]!     @ tmpQmf_c1[2*(n + 192)] load and incremented


    ADD             R2, R2, #112        @ This should be 112
    VLD1.16         D8, [R0], R6        @ tmpQ1[n + 256] load and incremented by R6
    VMLAL.S16       Q15, D6, D7

    MOV             R0, R9
    VLD1.16         {D9, D10}, [R2]!    @ tmpQmf_c1[2*(n + 256)] load and incremented


    ADD             R2, R2, #112        @ This should be 112
    VLD1.16         D10, [R1]!          @ tmpQ2[n +   0] load and incremented
    VMLAL.S16       Q15, D8, D9



    MOV             R9, R1
    VLD1.16         {D11, D12}, [R3]!   @ tmpQmf_c2[2*(n +   0)] load and incremented
    ADD             R1, R1, #120        @ incrementing R1 by 120 + 8 = 128


    MOV             R2, R10             @
    VLD1.16         D12, [R1], R6       @ tmpQ2[n +  64] load and incremented by R6
    MOV             R10, R3

    ADD             R3, R3, #112        @ This sholud be 112
    VLD1.16         {D13, D14}, [R3]!   @ tmpQmf_c2[2*(n +  64)] load and incremented
    ADD             R3, R3, #112        @ This sholud be 112


    VLD1.16         {D15, D16}, [R3]!   @ tmpQmf_c2[2*(n +  128)] load and incremented

    SUB             R10, R10, #8

    VLD1.16         D14, [R1], R6
    ADD             R3, R3, #112        @ This should be 112



    VLD1.16         D16, [R1], R6
    SUB             R5, R5, #1

    VLD1.16         {D17, D18}, [R3]!   @ tmpQmf_c2[2*(n +  192)] load and incremented


    ADD             R3, R3, #112        @ This should be 112
    VLD1.16         D18, [R1], R6

    MOV             R1, R9
    VLD1.16         {D19, D20}, [R3]!   @ tmpQmf_c2[2*(n +  256)] load and incremented

    ADD             R3, R3, #112        @ This should be 112

    MOV             R3, R10


LOOP_1:


    VLD1.16         D0, [R0]!

    MOV             R9, R0
    VLD1.16         {D1, D2}, [R2]!
    ADD             R0, R0, #120

    MOV             R10, R2
    VST1.32         {Q15}, [R4]!
    ADD             R2, R2, #112        @ This should be 112


    VMULL.S16       Q15, D10, D11
    VLD1.16         D2, [R0], R6
    VMLAL.S16       Q15, D12, D13

    VMLAL.S16       Q15, D14, D15
    VLD1.16         {D3, D4}, [R2]!
    VMLAL.S16       Q15, D16, D17

    VMLAL.S16       Q15, D18, D19
    VLD1.16         D4, [R0], R6
    ADD             R2, R2, #112        @ This should be 112

    VST1.32         {Q15}, [R11]!
    SUB             R10, R10, #8


    VMULL.S16       Q15, D0, D1
    VLD1.16         {D5, D6}, [R2]!
    VMLAL.S16       Q15, D2, D3



    ADD             R2, R2, #112        @ This should be 112
    VLD1.16         D6, [R0], R6
    VMLAL.S16       Q15, D4, D5

    VLD1.16         {D7, D8}, [R2]!


    ADD             R2, R2, #112        @ This should be 112
    VLD1.16         D8, [R0], R6
    VMLAL.S16       Q15, D6, D7

    MOV             R0, R9
    VLD1.16         {D9, D10}, [R2]!



    ADD             R2, R2, #112        @ This should be 112
    VLD1.16         D10, [R1]!
    MOV             R2, R10

    MOV             R9, R1
    VLD1.16         {D11, D12}, [R3]!
    ADD             R1, R1, #120


    VMLAL.S16       Q15, D8, D9
    VLD1.16         D12, [R1], R6
    MOV             R10, R3


    ADD             R3, R3, #112        @ This should be 112
    VLD1.16         {D13, D14}, [R3]!
    ADD             R3, R3, #112        @ This should be 112



    VLD1.16         D14, [R1], R6
    SUB             R10, R10, #8
    VLD1.16         {D15, D16}, [R3]!
    ADD             R3, R3, #112        @ This should be 112


    VLD1.16         D16, [R1], R6
    VLD1.16         {D17, D18}, [R3]!
    ADD             R3, R3, #112        @ This should be 112


    VLD1.16         D18, [R1], R6
    SUBS            R5, R5, #1

    MOV             R1, R9
    VLD1.16         {D19, D20}, [R3]!

    ADD             R3, R3, #112        @ This should be 112

    MOV             R3, R10

    BGT             LOOP_1

    VST1.32         {Q15}, [R4]!
    VMULL.S16       Q15, D10, D11
    VMLAL.S16       Q15, D12, D13

    VMLAL.S16       Q15, D14, D15
    VMLAL.S16       Q15, D16, D17
    VMLAL.S16       Q15, D18, D19

    VST1.32         {Q15}, [R11]!
    LDMFD           sp!, {R4-R12, R15}
