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
#ifndef __fileifc_h__
#define __fileifc_h__

#ifndef INCLUDE_MP4_PARSER
//#define INCLUDE_MP4_PARSER
#endif
#ifdef INCLUDE_MP4_PARSER
/* Context structure for media frame*/
typedef struct {
  unsigned char *frame;
  unsigned int frameLength;
  unsigned int presentationTime;
} ItMp4FrameCtxt;

/* Context structure for media header*/
typedef struct {
  unsigned char *header;
  unsigned int headerLength;
} ItMp4HeaderCtxt;
#endif

typedef struct FileWrapper {
#ifdef INCLUDE_MP4_PARSER

  void *mp4Ctxt;
  void *fileCtxt;
  void *interim_buffer;
  int avail_buffer;
  ItMp4HeaderCtxt headerCtxt;
  ItMp4FrameCtxt frameCtxt;
  int header_given;
#endif
  /*  MPEG_2_PROFILE profile; */
  unsigned int isMp4File;
  FILE *inputFile;

} FileWrapper, *FileWrapperPtr;

FileWrapperPtr FileWrapper_Open(char fileName[]);
int FileWrapper_Read(FileWrapperPtr transport, unsigned char *buffer,
                     int bufSize, unsigned int *len);
unsigned int FileWrapper_Close(FileWrapperPtr transport);

unsigned int FileWrapper_IsMp4File(FileWrapperPtr transport);

/* MP4 function declarations */

/*File Initialization, File Header Parsing*/
void *it_mp4_parser_init(void *mp4Ctxt, void *mallocAddr);

/*Extracts audio header from file*/
int it_mp4_get_audio_header(void *mp4Ctxt, void *audioheader);

/*Extracts an audio frame from the file */
int it_mp4_get_audio(void *mp4Ctxt, void *frameCtxt);

/*Frees allocated memory*/
int it_mp4_parser_close(void *mp4);

void *it_fopen(unsigned char *file_name, unsigned char withFile, int size);
int it_fclose(void *itf);
#endif
