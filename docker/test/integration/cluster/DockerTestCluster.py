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
import logging
import time
import re

from .LogSource import LogSource
from .ContainerStore import ContainerStore
from .DockerCommunicator import DockerCommunicator
from .checkers.AwsChecker import AwsChecker
from .checkers.AzureChecker import AzureChecker
from .checkers.ElasticSearchChecker import ElasticSearchChecker
from .checkers.GcsChecker import GcsChecker
from .checkers.PostgresChecker import PostgresChecker
from .checkers.PrometheusChecker import PrometheusChecker
from .checkers.SplunkChecker import SplunkChecker
from utils import get_peak_memory_usage, get_minifi_pid, get_memory_usage


class DockerTestCluster:
    def __init__(self, context):
        self.segfault = False
        self.vols = {}
        self.container_communicator = DockerCommunicator()
        self.container_store = ContainerStore(self.container_communicator.create_docker_network(), context.image_store, context.kubernetes_proxy)
        self.aws_checker = AwsChecker(self.container_communicator)
        self.azure_checker = AzureChecker(self.container_communicator)
        self.elastic_search_checker = ElasticSearchChecker(self.container_communicator)
        self.gcs_checker = GcsChecker(self.container_communicator)
        self.postgres_checker = PostgresChecker(self.container_communicator)
        self.splunk_checker = SplunkChecker(self.container_communicator)
        self.prometheus_checker = PrometheusChecker()

    def cleanup(self):
        self.container_store.cleanup()

    def set_directory_bindings(self, volumes, data_directories):
        self.container_store.set_directory_bindings(volumes, data_directories)

    def acquire_container(self, name, engine='minifi-cpp', command=None):
        return self.container_store.acquire_container(name, engine, command)

    def deploy_container(self, name):
        self.container_store.deploy_container(name)

    def deploy_all(self):
        self.container_store.deploy_all()

    def stop_container(self, container_name):
        self.container_store.stop_container(container_name)

    def kill_container(self, container_name):
        self.container_store.kill_container(container_name)

    def restart_container(self, container_name):
        self.container_store.restart_container(container_name)

    def enable_provenance_repository_in_minifi(self):
        self.container_store.enable_provenance_repository_in_minifi()

    def enable_c2_in_minifi(self):
        self.container_store.enable_c2_in_minifi()

    def enable_c2_with_ssl_in_minifi(self):
        self.container_store.enable_c2_with_ssl_in_minifi()

    def fetch_flow_config_from_c2_url_in_minifi(self):
        self.container_store.fetch_flow_config_from_c2_url_in_minifi()

    def enable_prometheus_in_minifi(self):
        self.container_store.enable_prometheus_in_minifi()

    def enable_sql_in_minifi(self):
        self.container_store.enable_sql_in_minifi()

    def set_yaml_in_minifi(self):
        self.container_store.set_yaml_in_minifi()

    def get_app_log(self, container_name):
        log_source = self.container_store.log_source(container_name)
        if log_source == LogSource.FROM_DOCKER_CONTAINER:
            return self.container_communicator.get_app_log_from_docker_container(container_name)
        elif log_source == LogSource.FROM_GET_APP_LOG_METHOD:
            return self.container_store.get_app_log(container_name)
        else:
            raise Exception("Unexpected log source '%s'" % log_source)

    def __wait_for_app_logs_impl(self, container_name, log_entry, timeout_seconds, count, use_regex):
        wait_start_time = time.perf_counter()
        while True:
            logging.info('Waiting for app-logs `%s` in container `%s`', log_entry, container_name)
            status, logs = self.get_app_log(container_name)
            if logs is not None:
                if not use_regex and logs.decode("utf-8").count(log_entry) >= count:
                    return True
                elif use_regex and len(re.findall(log_entry, logs.decode("utf-8"))) >= count:
                    return True
            elif status == 'exited':
                return False
            time.sleep(1)
            if timeout_seconds < (time.perf_counter() - wait_start_time):
                break
        return False

    def wait_for_app_logs_regex(self, container_name, log_entry, timeout_seconds, count=1):
        return self.__wait_for_app_logs_impl(container_name, log_entry, timeout_seconds, count, True)

    def wait_for_app_logs(self, container_name, log_entry, timeout_seconds, count=1):
        return self.__wait_for_app_logs_impl(container_name, log_entry, timeout_seconds, count, False)

    def wait_for_startup_log(self, container_name, timeout_seconds):
        return self.wait_for_app_logs_regex(container_name, self.container_store.get_startup_finished_log_entry(container_name), timeout_seconds, 1)

    def log_app_output(self):
        for container_name in self.container_store.get_container_names():
            _, logs = self.get_app_log(container_name)
            if logs is not None:
                logging.info("Logs of container '%s':", container_name)
                for line in logs.decode("utf-8").splitlines():
                    logging.info(line)

    def check_http_proxy_access(self, container_name, url):
        (code, output) = self.container_communicator.execute_command(container_name, ["cat", "/var/log/squid/access.log"])
        return code == 0 and url in output \
            and ((output.count("TCP_DENIED") != 0
                 and output.count("TCP_MISS") >= output.count("TCP_DENIED"))
                 or output.count("TCP_DENIED") == 0 and "TCP_MISS" in output)

    def check_s3_server_object_data(self, container_name, test_data):
        return self.aws_checker.check_s3_server_object_data(container_name, test_data)

    def check_s3_server_object_metadata(self, container_name, content_type="application/octet-stream", metadata=dict()):
        return self.aws_checker.check_s3_server_object_metadata(container_name, content_type, metadata)

    def is_s3_bucket_empty(self, container_name):
        return self.aws_checker.is_s3_bucket_empty(container_name)

    def check_azure_storage_server_data(self, container_name, test_data):
        return self.azure_checker.check_azure_storage_server_data(container_name, test_data)

    def add_test_blob(self, blob_name, content="", with_snapshot=False):
        return self.azure_checker.add_test_blob(blob_name, content, with_snapshot)

    def check_azure_blob_and_snapshot_count(self, blob_and_snapshot_count, timeout_seconds):
        return self.azure_checker.check_azure_blob_and_snapshot_count(blob_and_snapshot_count, timeout_seconds)

    def check_azure_blob_storage_is_empty(self, timeout_seconds):
        return self.azure_checker.check_azure_blob_storage_is_empty(timeout_seconds)

    def check_splunk_event(self, container_name, query):
        return self.splunk_checker.check_splunk_event(container_name, query)

    def check_splunk_event_with_attributes(self, container_name, query, attributes):
        return self.splunk_checker.check_splunk_event_with_attributes(container_name, query, attributes)

    def enable_splunk_hec_indexer(self, container_name, hec_name):
        return self.splunk_checker.enable_splunk_hec_indexer(container_name, hec_name)

    def enable_splunk_hec_ssl(self, container_name, splunk_cert_pem, splunk_key_pem, root_ca_cert_pem):
        return self.splunk_checker.enable_splunk_hec_ssl(container_name, splunk_cert_pem, splunk_key_pem, root_ca_cert_pem)

    def check_google_cloud_storage(self, gcs_container_name, content):
        return self.gcs_checker.check_google_cloud_storage(gcs_container_name, content)

    def is_gcs_bucket_empty(self, container_name):
        return self.gcs_checker.is_gcs_bucket_empty(container_name)

    def is_elasticsearch_empty(self, container_name):
        return self.elastic_search_checker.is_elasticsearch_empty(container_name)

    def create_doc_elasticsearch(self, container_name, index_name, doc_id):
        return self.elastic_search_checker.create_doc_elasticsearch(container_name, index_name, doc_id)

    def check_elastic_field_value(self, container_name, index_name, doc_id, field_name, field_value):
        return self.elastic_search_checker.check_elastic_field_value(container_name, index_name, doc_id, field_name, field_value)

    def elastic_generate_apikey(self, elastic_container_name):
        return self.elastic_search_checker.elastic_generate_apikey(elastic_container_name)

    def add_elastic_user_to_opensearch(self, container_name):
        return self.elastic_search_checker.add_elastic_user_to_opensearch(container_name)

    def check_query_results(self, postgresql_container_name, query, number_of_rows, timeout_seconds):
        return self.postgres_checker.check_query_results(postgresql_container_name, query, number_of_rows, timeout_seconds)

    def segfault_happened(self):
        return self.segfault

    def wait_for_kafka_consumer_to_be_registered(self, kafka_container_name):
        return self.wait_for_app_logs(kafka_container_name, "Assignment received from leader for group docker_test_group", 60)

    def wait_for_metric_class_on_prometheus(self, metric_class, timeout_seconds):
        return self.prometheus_checker.wait_for_metric_class_on_prometheus(metric_class, timeout_seconds)

    def wait_for_processor_metric_on_prometheus(self, metric_class, timeout_seconds, processor_name):
        return self.prometheus_checker.wait_for_processor_metric_on_prometheus(metric_class, timeout_seconds, processor_name)

    def check_minifi_log_matches_regex(self, regex, timeout_seconds=60, count=1):
        for container_name in self.container_store.get_container_names("minifi-cpp"):
            line_found = self.wait_for_app_logs_regex(container_name, regex, timeout_seconds, count)
            if line_found:
                return True
        return False

    def check_container_log_contents(self, container_engine, line, timeout_seconds=60, count=1):
        for container_name in self.container_store.get_container_names(container_engine):
            line_found = self.wait_for_app_logs(container_name, line, timeout_seconds, count)
            if line_found:
                return True
        return False

    def check_minifi_log_does_not_contain(self, line, wait_time_seconds):
        time.sleep(wait_time_seconds)
        for container_name in self.container_store.get_container_names("minifi-cpp"):
            _, logs = self.get_app_log(container_name)
            if logs is not None and 1 <= logs.decode("utf-8").count(line):
                return False
        return True

    def wait_for_container_startup_to_finish(self, container_name):
        startup_success = self.wait_for_startup_log(container_name, 120)
        if not startup_success:
            logging.error("Cluster startup failed for %s", container_name)
            self.log_app_output()
        return startup_success

    def wait_for_all_containers_to_finish_startup(self):
        for container_name in self.container_store.get_container_names():
            if not self.wait_for_container_startup_to_finish(container_name):
                return False
        return True

    def wait_for_peak_memory_usage_to_exceed(self, minimum_peak_memory_usage: int, timeout_seconds: int) -> bool:
        start_time = time.perf_counter()
        while (time.perf_counter() - start_time) < timeout_seconds:
            current_peak_memory_usage = get_peak_memory_usage(get_minifi_pid())
            if current_peak_memory_usage is None:
                logging.warning("Failed to determine peak memory usage")
                return False
            if current_peak_memory_usage > minimum_peak_memory_usage:
                return True
            time.sleep(1)
        logging.warning(f"Peak memory usage ({current_peak_memory_usage}) didnt exceed minimum asserted peak memory usage {minimum_peak_memory_usage}")
        return False

    def wait_for_memory_usage_to_drop_below(self, max_memory_usage: int, timeout_seconds: int) -> bool:
        start_time = time.perf_counter()
        while (time.perf_counter() - start_time) < timeout_seconds:
            current_memory_usage = get_memory_usage(get_minifi_pid())
            if current_memory_usage is None:
                logging.warning("Failed to determine memory usage")
                return False
            if current_memory_usage < max_memory_usage:
                return True
            current_memory_usage = get_memory_usage(get_minifi_pid())
            time.sleep(1)
        logging.warning(f"Memory usage ({current_memory_usage}) is more than the maximum asserted memory usage ({max_memory_usage})")
        return False