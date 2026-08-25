/******************************************************************************
 *                                                                            *
 * Copyright (C) 2018 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at:
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 *****************************************************************************
 * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
*/
#include <stdlib.h>
#include <string.h>
#ifndef CORTEX_M4
#include <stdio.h>
#else
#include "cm4_file_operations.h"
#endif
#include <assert.h>

#include "ixheaacd_fileifc.h"
#ifndef min
#define min(a, b) (((a) < (b)) ? (a) : (b))
#endif

//#define INCLUDE_MP4_PARSER

FileWrapperPtr FileWrapper_Open(char fileName[]) {
  FileWrapperPtr transport = (FileWrapper *)calloc(1, sizeof(FileWrapper));

  transport->isMp4File = 0;
  transport->inputFile = NULL;

#ifdef INCLUDE_MP4_PARSER

  transport->header_given = 0;
  transport->fileCtxt = 0;
  transport->mp4Ctxt = 0;
  transport->interim_buffer = 0;
  transport->avail_buffer = 0;

  if ((transport->fileCtxt = it_fopen((void *)fileName, 1, 0)) == NULL) {
    transport->inputFile = fopen(fileName, "rb");
    if (!transport->inputFile) {
      free(transport);
      return 0;
    }

    else
      return transport;
  }

  if ((transport->mp4Ctxt = it_mp4_parser_init(transport->fileCtxt, NULL)) ==
      NULL) {
    transport->inputFile = fopen(fileName, "rb");
    if (!transport->inputFile) {
      free(transport);
      return 0;
    } else
      return transport;
  }
  transport->isMp4File = 1;
  /* 768 * max channels is the max audio block size for AAC */
  /* As max channels is 8 and +2 for upto two Ind CoupCh */
  transport->interim_buffer = malloc(10 * 768);
  if (transport->interim_buffer == NULL) {
    free(transport);
    return 0;
  }
  return transport;
#else
  transport->inputFile = fopen(fileName, "rb");
  if (!transport->inputFile) {
    free(transport);
    return 0;
  } else
    return transport;
#endif
}

int FileWrapper_Read(FileWrapperPtr transport, unsigned char *buffer,
                     int bufSize, unsigned int *length) {
#ifdef INCLUDE_MP4_PARSER
  if (!transport->isMp4File)
    *length = fread(buffer, 1, bufSize, transport->inputFile);
  else {
    int errorCode;
    if (transport->header_given == 0) {
      transport->headerCtxt.header = (void *)buffer;
      /* MP4 Hook: Audio Header data to be put in buffer pointed by
      // headerCtxt.header and length to be put in headerCtxt.headerLength*/
      errorCode =
          it_mp4_get_audio_header(transport->mp4Ctxt, &transport->headerCtxt);
      transport->header_given = 1;
      if (errorCode != 0) /* IT_OK = 0 */
      {
        *length = 0;
        return 1;
      }
      *length = transport->headerCtxt.headerLength;
    } else {
      *length = 0;
/* MP4 Hook: Audio Data to be put in buffer pointed by frameCtxt.header*/
#ifndef ENABLE_LD_DEC
      while ((int)*length < bufSize)
#endif
      {
        if (transport->avail_buffer == 0) {
          transport->frameCtxt.frame = (void *)transport->interim_buffer;
          errorCode =
              it_mp4_get_audio(transport->mp4Ctxt, &transport->frameCtxt);
          if (errorCode != 0) /* IT_OK = 0 */
          {
            if (*length == 0)
              return 1;
            else
#ifndef ENABLE_LD_DEC
              break;
#else
              return 1;
#endif
          }
          transport->avail_buffer += transport->frameCtxt.frameLength;
        }
#ifndef ENABLE_LD_DEC
        else
#endif
        {
          int size_to_cpy =
              min((int)(bufSize - *length), transport->avail_buffer);
          memcpy(buffer + (*length), transport->interim_buffer, size_to_cpy);
          memmove(transport->interim_buffer,
                  ((char *)transport->interim_buffer) + size_to_cpy,
                  (transport->avail_buffer - size_to_cpy));
          *length += size_to_cpy;
          transport->avail_buffer -= size_to_cpy;
        }
      }
    }
#ifdef DEBUG_ASSERT
    assert(*length <= (unsigned)bufSize);
#endif
  }

#else
  *length = (unsigned int)fread(buffer, 1, bufSize, transport->inputFile);
#endif

  return 0;
}

unsigned int FileWrapper_Close(FileWrapperPtr transport) {
  if (transport == 0) return 0;

#ifdef INCLUDE_MP4_PARSER
  if (transport->isMp4File) {
    it_mp4_parser_close(transport->mp4Ctxt);
    it_fclose(transport->fileCtxt);
  } else
#endif
      if (transport->inputFile)
    fclose(transport->inputFile);

  free(transport);
  return 0;
}

unsigned int FileWrapper_IsMp4File(FileWrapperPtr transport) {
  return transport->isMp4File;
}
