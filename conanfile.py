from conans import ConanFile, CMake

class MinificppConan(ConanFile):
    name = "nifi-minifi-cpp"
    license = "Apache-2.0"
    homepage = "https://nifi.apache.org/minifi"
    settings = "os", "compiler", "build_type", "arch"
    options = {
            "portable": [True, False],
            "with_rocksdb": [True, False],
            #"with_curl": [True, False],
            #"with_libarchive": [True, False],
            "with_opencv": [True, False],
    }
    default_options = {
            "portable": True,
            "with_rocksdb": True,
            "with_opencv": False,

            "rocksdb:use_rtti": True,
            "opencv:with_tiff": False, 
            "opencv:with_jpeg2000": False,
            "opencv:with_openexr": False,
            "opencv:with_eigen": False,
            "opencv:with_webp": False,
            "opencv:with_gtk": False,
            "opencv:with_quirc": False
    }

    #requires = "rocksdb/6.20.3,opencv/4.5.2" # comma-separated list of requirements
    generators = "cmake"

    def imports(self):
        self.copy("*.dll", dst="bin", src="bin") # From bin to bin
        self.copy("*.dylib*", dst="bin", src="lib") # From lib to bin

    def build(self):
        cmake = CMake(self)

        cmake.definitions["PORTABLE"] = self.options.get_safe("portable", False)
        cmake.definitions["AWS_ENABLE_UNITY_BUILD"] = False
        cmake.definitions["ASAN_BUILD"] = False
        cmake.definitions["STATIC_BUILD"] = True

        cmake.definitions["ENABLE_ROCKSDB"] = self.options.get_safe("with_rocksdb", False)
        cmake.definitions["BUILD_ROCKSDB"] = False
        cmake.definitions["ENABLE_OPENCV"] = self.options.get_safe("with_opencv", False)
        cmake.definitions["ENABLE_PYTHON"] = False
        cmake.definitions["ENABLE_ENCRYPT_CONFIG"] = True
        cmake.definitions["ENABLE_OPC"] = True
        cmake.definitions["ENABLE_JNI"] = True
        cmake.definitions["ENABLE_OPC"] = True
        cmake.definitions["ENABLE_COAP"] = True
        cmake.definitions["ENABLE_GPS"] = True
        cmake.definitions["ENABLE_MQTT"] = True
        cmake.definitions["ENABLE_LIBRDKAFKA"] = True
        cmake.definitions["ENABLE_SENSORS"] = True
        cmake.definitions["ENABLE_USB_CAMERA"] = True
        cmake.definitions["ENABLE_AWS"] = False
        cmake.definitions["ENABLE_SFTP"] = False
        cmake.definitions["ENABLE_OPENWSMAN"] = True
        cmake.definitions["ENABLE_BUSTACHE"] = True
        cmake.definitions["ENABLE_TENSORFLOW"] = False
        cmake.definitions["ENABLE_SQL"] = False
        cmake.definitions["ENABLE_PCAP"] = False
        cmake.definitions["ENABLE_NANOFI"] = True
        cmake.definitions["ENABLE_SYSTEMD"] = True

        cmake.configure()
        cmake.build()

    def requirements(self):
        self.requires("spdlog/1.8.5")
        if self.options.with_rocksdb:
            self.requires("rocksdb/6.20.3")
        if self.options.with_opencv:
            self.requires("opencv/4.5.2")
