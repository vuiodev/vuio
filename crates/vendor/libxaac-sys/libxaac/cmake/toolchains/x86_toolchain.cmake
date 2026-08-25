# This one is important
SET(CMAKE_SYSTEM_NAME Linux)
SET(CMAKE_SYSTEM_PROCESSOR i686)

# Specify the cross compiler
SET(CMAKE_C_COMPILER   gcc)
SET(CMAKE_CXX_COMPILER g++)

# Where is the target environment
SET(CMAKE_FIND_ROOT_PATH  /)

# Search for programs in the build host directories
SET(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)

# For libraries and headers in the target directories
SET(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
SET(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)

