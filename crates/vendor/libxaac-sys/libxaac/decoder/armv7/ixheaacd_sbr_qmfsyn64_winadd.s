@/******************************************************************************
@ *
@ * Copyright (C) 2018 The Android Open Source Project
@ *
@ * Licensed under the Apache License, Version 2.0 (the "License");
@ * you may not use this file except in compliance with the License.
@ * You may obtain a copy of the License at:
@ *
@ * http://www.apache.org/licenses/LICENSE-2.0
@ *
@ * Unless required by applicable law or agreed to in writing, software
@ * distributed under the License is distributed on an "AS IS" BASIS,
@ * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
@ * See the License for the specific language governing permissions and
@ * limitations under the License.
@ *
@ *****************************************************************************
@ * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
@*/


.text
.p2align 2
      .global ixheaacd_sbr_qmfsyn64_winadd
      .type ixheaacd_sbr_qmfsyn64_winadd, %function

ixheaacd_sbr_qmfsyn64_winadd:

    STMFD           sp!, {R4-R12, R14}
    VPUSH           {D8- D15}
    LDR             R4, [SP, #104]
    LDR             R5, [SP, #108]

    MOV             R7, #0x8000
    VLD1.16         D0, [R0]!
    MOV             R12, R2

    VDUP.32         Q15, R7
    VLD1.16         D1, [R2]!
    VDUP.32         Q11, R4

    MOV             R10, R0
    MOV             R11, R2
    ADD             R0, R0, #504
    ADD             R2, R2, #248

    VNEG.S32        Q14, Q11
    VSHL.S32        Q10, Q15, Q14
    MOV             R6, #64
    MOV             R6, R6, LSL #1
    ADD             R12, R12, R6
    MOV             R7, #128
    MOV             R9, R7, LSL #1
    ADD             R1, R1, R9
    MOV             R6, #16
    MOV             R7, #128
    MOV             R9, R7, LSL #1
    MOV             R7, #256
    MOV             R8, R7, LSL #1

    MOV             R5, R5, LSL #1
    VLD1.16         D2, [R0], R8
    VMOV            Q13, Q10


    VMLAL.S16       Q13, D0, D1
    VLD1.16         D3, [R2], R9

    VLD1.16         D4, [R0], R8
    VMLAL.S16       Q13, D2, D3

    VLD1.16         D5, [R2], R9

    VLD1.16         D6, [R0], R8
    VMLAL.S16       Q13, D5, D4

    VLD1.16         D7, [R2], R9

    VLD1.16         D8, [R0], R8
    VMLAL.S16       Q13, D7, D6

    VLD1.16         D9, [R2], R9
    MOV             R0, R10


    MOV             R2, R11
    VLD1.16         D10, [R1]!
    VMLAL.S16       Q13, D9, D8

    MOV             R10, R1
    VLD1.16         D11, [R12]!
    ADD             R1, R1, #504



    MOV             R11, R12
    VLD1.16         D12, [R1], R8
    ADD             R12, R12, #248

    VMLAL.S16       Q13, D10, D11
    VLD1.16         D13, [R12], R9

    VLD1.16         D14, [R1], R8
    VMLAL.S16       Q13, D12, D13

    VLD1.16         D15, [R12], R9

    VLD1.16         D16, [R1], R8
    VMLAL.S16       Q13, D15, D14

    VLD1.16         D17, [R12], R9

    VLD1.16         D18, [R1], R8
    VMLAL.S16       Q13, D17, D16

    VLD1.16         D19, [R12], R9

    VMLAL.S16       Q13, D19, D18
    VLD1.16         D0, [R0]!
    MOV             R12, R11

    MOV             R1, R10
    VLD1.16         D1, [R2]!
    MOV             R10, R0

    VQSHL.S32       Q13, Q13, Q11

    ADD             R0, R0, #504

    MOV             R11, R2
    VLD1.16         D2, [R0], R8
    ADD             R2, R2, #248

    VSHR.S32        Q14, Q13, #16
    VLD1.16         D3, [R2], R9


    VUZP.16         D28, D29
    VMOV            Q13, Q10





    VLD1.16         D4, [R0], R8
    VLD1.16         D5, [R2], R9

    VLD1.16         D6, [R0], R8
    VLD1.16         D7, [R2], R9

    VLD1.16         D8, [R0], R8
    VLD1.16         D9, [R2], R9
    MOV             R0, R10


    MOV             R2, R11
    VLD1.16         D10, [R1]!

    MOV             R10, R1
    VLD1.16         D11, [R12]!
    ADD             R1, R1, #504


    MOV             R11, R12
    VLD1.16         D12, [R1], R8
    ADD             R12, R12, #248


    VLD1.16         D13, [R12], R9

    VLD1.16         D14, [R1], R8
    VLD1.16         D15, [R12], R9

    VLD1.16         D16, [R1], R8
    VLD1.16         D17, [R12], R9

    VLD1.16         D18, [R1], R8
    SUB             R6, R6, #2
    VLD1.16         D19, [R12], R9
    MOV             R1, R10

    MOV             R12, R11

LOOP_1:

    VMLAL.S16       Q13, D0, D1
    VST1.16         D28[0], [R3], R5

    VMLAL.S16       Q13, D2, D3
    VLD1.16         D0, [R0]!
    VMLAL.S16       Q13, D5, D4

    VMLAL.S16       Q13, D7, D6
    VST1.16         D28[1], [R3], R5


    MOV             R10, R0
    VLD1.16         D1, [R2]!
    ADD             R0, R0, #504

    VMLAL.S16       Q13, D9, D8
    VST1.16         D28[2], [R3], R5

    VMLAL.S16       Q13, D10, D11
    VST1.16         D28[3], [R3], R5

    MOV             R11, R2
    VLD1.16         D2, [R0], R8
    ADD             R2, R2, #248

    VMLAL.S16       Q13, D12, D13
    VLD1.16         D3, [R2], R9
    VMLAL.S16       Q13, D15, D14

    VMLAL.S16       Q13, D17, D16
    VLD1.16         D4, [R0], R8
    VMLAL.S16       Q13, D19, D18

    VLD1.16         D5, [R2], R9

    VLD1.16         D6, [R0], R8
    VQSHL.S32       Q13, Q13, Q11

    VSHR.S32        Q14, Q13, #16
    VLD1.16         D7, [R2], R9
    VMOV            Q13, Q10


    VUZP.16         D28, D29
    VMLAL.S16       Q13, D0, D1

    VMLAL.S16       Q13, D2, D3
    VLD1.16         D8, [R0], R8
    VMLAL.S16       Q13, D5, D4

    VMLAL.S16       Q13, D7, D6
    VLD1.16         D9, [R2], R9


    VLD1.16         D10, [R1]!
    VMLAL.S16       Q13, D9, D8

    MOV             R2, R11
    VLD1.16         D11, [R12]!
    MOV             R0, R10

    MOV             R10, R1

    ADD             R1, R1, #504

    MOV             R11, R12
    VLD1.16         D12, [R1], R8
    ADD             R12, R12, #248

    VLD1.16         D13, [R12], R9
    VMLAL.S16       Q13, D10, D11

    VLD1.16         D14, [R1], R8
    VMLAL.S16       Q13, D12, D13

    VLD1.16         D15, [R12], R9

    VLD1.16         D16, [R1], R8
    VMLAL.S16       Q13, D15, D14

    VLD1.16         D17, [R12], R9

    VLD1.16         D18, [R1], R8
    VMLAL.S16       Q13, D17, D16

    VLD1.16         D19, [R12], R9
    MOV             R1, R10

    VMLAL.S16       Q13, D19, D18
    VST1.16         D28[0], [R3], R5

    MOV             R12, R11
    VLD1.16         D0, [R0]!

    VLD1.16         D1, [R2]!
    VQSHL.S32       Q13, Q13, Q11


    VST1.16         D28[1], [R3], R5
    MOV             R10, R0

    VST1.16         D28[2], [R3], R5
    ADD             R0, R0, #504

    VST1.16         D28[3], [R3], R5
    MOV             R11, R2

    VSHR.S32        Q14, Q13, #16
    VLD1.16         D2, [R0], R8
    ADD             R2, R2, #248

    VLD1.16         D3, [R2], R9
    VLD1.16         D4, [R0], R8
    VLD1.16         D5, [R2], R9
    VLD1.16         D6, [R0], R8
    VLD1.16         D7, [R2], R9
    VLD1.16         D8, [R0], R8
    VLD1.16         D9, [R2], R9

    VUZP.16         D28, D29
    VMOV            Q13, Q10




    MOV             R0, R10
    VLD1.16         D10, [R1]!
    MOV             R2, R11

    MOV             R10, R1
    VLD1.16         D11, [R12]!
    ADD             R1, R1, #504


    MOV             R11, R12
    VLD1.16         D12, [R1], R8
    ADD             R12, R12, #248


    VLD1.16         D13, [R12], R9

    VLD1.16         D14, [R1], R8
    VLD1.16         D15, [R12], R9

    VLD1.16         D16, [R1], R8
    VLD1.16         D17, [R12], R9

    SUBS            R6, R6, #2
    VLD1.16         D18, [R1], R8

    MOV             R1, R10
    VLD1.16         D19, [R12], R9

    MOV             R12, R11


    BGT             LOOP_1

    VMLAL.S16       Q13, D0, D1
    VST1.16         D28[0], [R3], R5
    VMLAL.S16       Q13, D2, D3

    VMLAL.S16       Q13, D5, D4
    VST1.16         D28[1], [R3], R5
    VMLAL.S16       Q13, D7, D6

    VMLAL.S16       Q13, D9, D8
    VST1.16         D28[2], [R3], R5
    VMLAL.S16       Q13, D10, D11

    VMLAL.S16       Q13, D12, D13
    VST1.16         D28[3], [R3], R5
    VMLAL.S16       Q13, D15, D14



    VMLAL.S16       Q13, D17, D16

    VMLAL.S16       Q13, D19, D18

    VQSHL.S32       Q13, Q13, Q11

    VSHR.S32        Q14, Q13, #16

    VUZP.16         D28, D29


    VST1.16         D28[0], [R3], R5
    VST1.16         D28[1], [R3], R5
    VST1.16         D28[2], [R3], R5
    VST1.16         D28[3], [R3], R5

    VPOP            {D8 - D15}
    LDMFD           sp!, {R4-R12, R15}

