# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements.  See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.
# The ASF licenses this file to You under the Apache License, Version 2.0
# (the "License"); you may not use this file except in compliance with
# the License.  You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.


import docker
import logging
import os
import shutil

from .KindProxy import KindProxy
from .LogSource import LogSource
from .MinifiContainer import MinifiContainer


class MinifiAsPodInKubernetesCluster(MinifiContainer):
    MINIFI_IMAGE_NAME = 'apacheminificpp'
    MINIFI_IMAGE_TAG = 'docker_test'

    def __init__(self, config_dir, name, vols, network, image_store, command=None):
        super().__init__(config_dir, name, vols, network, image_store, command)

        resources_directory = os.path.join(os.environ['TEST_DIRECTORY'], 'resources', 'kubernetes', 'pods-etc')
        self.kind = KindProxy(self.config_dir, resources_directory)

        test_dir = os.environ['TEST_DIRECTORY']
        shutil.copy(os.path.join(test_dir, os.pardir, os.pardir, os.pardir, 'conf', 'minifi.properties'), self.config_dir)
        shutil.copy(os.path.join(test_dir, 'resources', 'kubernetes', 'minifi-conf', 'minifi-log.properties'), self.config_dir)

        docker_client = docker.from_env()
        minifi_image = docker_client.images.get(MinifiAsPodInKubernetesCluster.MINIFI_IMAGE_NAME + ':' + os.environ['MINIFI_VERSION'])
        minifi_image.tag(MinifiAsPodInKubernetesCluster.MINIFI_IMAGE_NAME, MinifiAsPodInKubernetesCluster.MINIFI_IMAGE_TAG)

    def deploy(self):
        if not self.set_deployed():
            return

        logging.info('Setting up container: %s', self.name)

        self._create_config()

        self.kind.create_config(self.vols)
        self.kind.start_cluster()
        self.kind.load_docker_image(MinifiAsPodInKubernetesCluster.MINIFI_IMAGE_NAME, MinifiAsPodInKubernetesCluster.MINIFI_IMAGE_TAG)
        self.kind.create_objects()

        logging.info('Finished setting up container: %s', self.name)

    def log_source(self):
        return LogSource.FROM_GET_APP_LOG_METHOD

    def get_app_log(self):
        return 'OK', self.kind.get_logs('daemon', 'log-collector')

    def cleanup(self):
        logging.info('Cleaning up container: %s', self.name)
        self.kind.cleanup()
