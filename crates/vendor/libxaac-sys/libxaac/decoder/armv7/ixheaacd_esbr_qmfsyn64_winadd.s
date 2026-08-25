@VOID ixheaacd_esbr_qmfsyn64_winadd(
@WORD32 *tmp1,
@WORD32 *tmp2,
@WORD32 *inp1,
@WORD32 *sample_buffer,
@WORD32 ch_fac)
@R0->Word32 *tmp1
@R1->Word32 *tmp2
@R2->Word32 *inp1
@R3->Word32 *sample_buffer
@R5->ch_fac

.text
.p2align 2
      .global ixheaacd_esbr_qmfsyn64_winadd
      .type ixheaacd_esbr_qmfsyn64_winadd, %function

ixheaacd_esbr_qmfsyn64_winadd:          @ PROC

    STMFD           sp!, {R4-R12, R14}
    VPUSH           {D8- D15}
    LDR             R5, [SP, #104]

    MOV             R7, #0
    VLD1.32         {D0, D1}, [R0]!
    MOV             R12, R2

    VDUP.32         Q15, R7
    VLD1.32         {D2, D3}, [R2]!

    MOV             R10, R0
    MOV             R11, R2
    ADD             R0, R0, #1008
    ADD             R2, R2, #496

    MOV             R6, #64
    MOV             R6, R6, LSL #2
    ADD             R12, R12, R6
    MOV             R7, #256
    MOV             R9, R7, LSL #1
    ADD             R1, R1, R9
    MOV             R6, #64
    MOV             R7, #256
    MOV             R9, R7, LSL #1      @(256*2)
    MOV             R7, #512
    MOV             R8, R7, LSL #1      @(512*2)

    MOV             R5, R5, LSL #2
    VMOV            Q13, Q15
    VMOV            Q14, Q15

    VMLAL.S32       Q13, D0, D2
    VMLAL.S32       Q14, D1, D3

    VLD1.32         {D4, D5}, [R0], R8
    VLD1.32         {D6, D7}, [R2], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R0], R8
    VLD1.32         {D10, D11}, [R2], R9

    VMLAL.S32       Q13, D10, D8
    VMLAL.S32       Q14, D11, D9

    VLD1.32         {D12, D13}, [R0], R8
    VLD1.32         {D14, D15}, [R2], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R0], R8
    VLD1.32         {D18, D19}, [R2], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    MOV             R0, R10


    MOV             R2, R11
    VLD1.32         {D0, D1}, [R1]!
    MOV             R10, R1
    VLD1.32         {D2, D3}, [R12]!
    ADD             R1, R1, #1008
    MOV             R11, R12

    VMLAL.S32       Q13, D0, D2
    VMLAL.S32       Q14, D1, D3
    VLD1.32         {D4, D5}, [R1], R8
    ADD             R12, R12, #496

    VLD1.32         {D6, D7}, [R12], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R1], R8
    VLD1.32         {D10, D11}, [R12], R9

    VMLAL.S32       Q13, D10, D8
    VMLAL.S32       Q14, D11, D9

    VLD1.32         {D12, D13}, [R1], R8
    VLD1.32         {D14, D15}, [R12], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R1], R8
    VLD1.32         {D18, D19}, [R12], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    VSHRN.S64       D26 , Q13, #31

    VST1.32         D26[0], [R3], R5
    VST1.32         D26[1], [R3], R5

    VSHRN.S64       D27 , Q14, #31

    VST1.32         D27[0], [R3], R5
    VST1.32         D27[1], [R3], R5

    SUB             R6, R6, #8
LOOP_1:

    VLD1.32         {D0, D1}, [R0]!
    MOV             R12, R11
    MOV             R1, R10
    VLD1.32         {D2, D3}, [R2]!
    MOV             R10, R0

    ADD             R0, R0, #1008

    MOV             R11, R2
    ADD             R2, R2, #496


    VMOV            Q13, Q15
    VMOV            Q14, Q15

    VMLAL.S32       Q13, D0, D2
    VMLAL.S32       Q14, D1, D3

    VLD1.32         {D4, D5}, [R0], R8
    VLD1.32         {D6, D7}, [R2], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R0], R8
    VLD1.32         {D10, D11}, [R2], R9

    VMLAL.S32       Q13, D10, D8
    VMLAL.S32       Q14, D11, D9

    VLD1.32         {D12, D13}, [R0], R8
    VLD1.32         {D14, D15}, [R2], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R0], R8
    VLD1.32         {D18, D19}, [R2], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    MOV             R0, R10


    MOV             R2, R11
    VLD1.32         {D0, D1}, [R1]!
    MOV             R10, R1
    VLD1.32         {D2, D3}, [R12]!
    ADD             R1, R1, #1008
    MOV             R11, R12

    VMLAL.S32       Q13, D0, D2
    VMLAL.S32       Q14, D1, D3
    VLD1.32         {D4, D5}, [R1], R8
    ADD             R12, R12, #496

    VLD1.32         {D6, D7}, [R12], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R1], R8
    VLD1.32         {D10, D11}, [R12], R9

    VMLAL.S32       Q13, D10, D8
    VMLAL.S32       Q14, D11, D9

    VLD1.32         {D12, D13}, [R1], R8
    VLD1.32         {D14, D15}, [R12], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R1], R8
    VLD1.32         {D18, D19}, [R12], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    VSHRN.S64       D26 , Q13, #31

    VST1.32         D26[0], [R3], R5
    VST1.32         D26[1], [R3], R5

    VSHRN.S64       D27 , Q14, #31

    VST1.32         D27[0], [R3], R5
    VST1.32         D27[1], [R3], R5
@@@
    VLD1.32         {D0, D1}, [R0]!
    MOV             R12, R11
    MOV             R1, R10
    VLD1.32         {D2, D3}, [R2]!
    MOV             R10, R0

    VMOV            Q13, Q15
    VMLAL.S32       Q13, D0, D2
    VMOV            Q14, Q15
    VMLAL.S32       Q14, D1, D3

    ADD             R0, R0, #1008

    MOV             R11, R2
    VLD1.32         {D4, D5}, [R0], R8
    ADD             R2, R2, #496


    VLD1.32         {D6, D7}, [R2], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R0], R8
    VLD1.32         {D10, D11}, [R2], R9

    VMLAL.S32       Q13, D8, D10
    VMLAL.S32       Q14, D9, D11

    VLD1.32         {D12, D13}, [R0], R8
    VLD1.32         {D14, D15}, [R2], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R0], R8
    VLD1.32         {D18, D19}, [R2], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    MOV             R0, R10


    MOV             R2, R11
    VLD1.32         {D0, D1}, [R1]!

    MOV             R10, R1
    VLD1.32         {D2, D3}, [R12]!
    ADD             R1, R1, #1008

    VMLAL.S32       Q13, D0, D2
    VMLAL.S32       Q14, D1, D3

    MOV             R11, R12
    VLD1.32         {D4, D5}, [R1], R8
    ADD             R12, R12, #496


    VLD1.32         {D6, D7}, [R12], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R1], R8
    VLD1.32         {D10, D11}, [R12], R9

    VMLAL.S32       Q13, D8, D10
    VMLAL.S32       Q14, D9, D11

    VLD1.32         {D12, D13}, [R1], R8
    VLD1.32         {D14, D15}, [R12], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R1], R8
    VLD1.32         {D18, D19}, [R12], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    VSHRN.S64       D26 , Q13, #31

    VST1.32         D26[0], [R3], R5
    VST1.32         D26[1], [R3], R5

    VSHRN.S64       D27 , Q14, #31

    VST1.32         D27[0], [R3], R5
    VST1.32         D27[1], [R3], R5

    SUBS            R6, R6, #8          @1

    BGT             LOOP_1

    VLD1.32         {D0, D1}, [R0]!
    MOV             R12, R11
    MOV             R1, R10
    VLD1.32         {D2, D3}, [R2]!
    MOV             R10, R0

    VMOV            Q13, Q15
    VMLAL.S32       Q13, D0, D2
    VMOV            Q14, Q15
    VMLAL.S32       Q14, D1, D3

    ADD             R0, R0, #1008

    MOV             R11, R2
    VLD1.32         {D4, D5}, [R0], R8
    ADD             R2, R2, #496


    VLD1.32         {D6, D7}, [R2], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R0], R8
    VLD1.32         {D10, D11}, [R2], R9

    VMLAL.S32       Q13, D8, D10
    VMLAL.S32       Q14, D9, D11

    VLD1.32         {D12, D13}, [R0], R8
    VLD1.32         {D14, D15}, [R2], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R0], R8
    VLD1.32         {D18, D19}, [R2], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    MOV             R0, R10


    MOV             R2, R11
    VLD1.32         {D0, D1}, [R1]!

    MOV             R10, R1
    VLD1.32         {D2, D3}, [R12]!
    ADD             R1, R1, #1008

    VMLAL.S32       Q13, D0, D2
    VMLAL.S32       Q14, D1, D3

    MOV             R11, R12
    VLD1.32         {D4, D5}, [R1], R8
    ADD             R12, R12, #496


    VLD1.32         {D6, D7}, [R12], R9

    VMLAL.S32       Q13, D6, D4
    VMLAL.S32       Q14, D7, D5

    VLD1.32         {D8, D9}, [R1], R8
    VLD1.32         {D10, D11}, [R12], R9

    VMLAL.S32       Q13, D8, D10
    VMLAL.S32       Q14, D9, D11

    VLD1.32         {D12, D13}, [R1], R8
    VLD1.32         {D14, D15}, [R12], R9

    VMLAL.S32       Q13, D12, D14
    VMLAL.S32       Q14, D13, D15

    VLD1.32         {D16, D17}, [R1], R8
    VLD1.32         {D18, D19}, [R12], R9

    VMLAL.S32       Q13, D16, D18
    VMLAL.S32       Q14, D17, D19

    VSHRN.S64       D26 , Q13, #31

    VST1.32         D26[0], [R3], R5
    VST1.32         D26[1], [R3], R5

    VSHRN.S64       D27, Q14, #31

    VST1.32         D27[0], [R3], R5
    VST1.32         D27[1], [R3], R5

    VPOP            {D8 - D15}
    LDMFD           sp!, {R4-R12, R15}
    @ ENDP
