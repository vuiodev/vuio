SET(CMAKE_SYSTEM_NAME Linux)
SET(CMAKE_SYSTEM_PROCESSOR aarch64)

# Modify these variables with paths to appropriate compilers that can produce
# armv8 targets

# Specify the cross compiler
SET(CMAKE_C_COMPILER aarch64-linux-gnu-gcc)
SET(CMAKE_CXX_COMPILER aarch64-linux-gnu-g++)

# Where is the target environment
SET(CMAKE_FIND_ROOT_PATH  aarch64-linux-gnu-gnueabihf)

# Search for programs in the build host directories
SET(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)

# For libraries and headers in the target directories
SET(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
SET(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
