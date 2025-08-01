.. _configure_audit_value_types:

*****
Audit
*****

All audit value types are described here.

.. _conf_value_audit_icap_service_config:

icap service config
===================

**type**: map | str

Config ICAP service.

For *str* value, the value will be treated as *url* as described following.

For *map* value, the keys are:

* url

  **required**, **type**: :ref:`url str <conf_value_url_str>`

  Set the ICAP service url. The scheme should be either 'icap' or 'icaps'.
  A default tls client config will be used if the scheme is 'icaps'.

* use_unix_socket

  **optional**, **type**: :ref:`absolute path <conf_value_absolute_path>`

  Set a UNIX domain socket address to use before try via TCP.

  If the path can't be connected the TCP address in the url will be used as a fallback.

  **default**: not set

  .. versionadded:: 1.12.0

* tls_client

  **optional**, **type**: :ref:`rustls client config <conf_value_rustls_client_config>`

  Enable tls and set the config. TLS will be enabled even if the url scheme is 'icap'.

  **default**: not set for 'icap://' url, default one for 'icaps://' url

  .. versionadded:: 1.9.9

* tls_name

  **optional**, **type**: :ref:`tls name <conf_value_tls_name>`

  Set the tls server name to verify peer certificate.

  **default**: same as the host port in url

  .. versionadded:: 1.9.9

* tcp_keepalive

  **optional**, **type**: :ref:`tcp keepalive <conf_value_tcp_keepalive>`

  Set the keep-alive config for the tcp connection to ICAP server.

  **default**: enabled with default value

* icap_connection_pool

  **optional**, **type**: :ref:`connection pool <conf_value_connection_pool_config>`

  Set the connection pool config.

  **default**: set with default value

* icap_max_header_size

  **optional**, **type**: :ref:`humanize usize <conf_value_humanize_usize>`

  Set the max header size when parsing response from the ICAP server.

  **default**: 8KiB

* no_preview

  **optional**, **type**: bool

  Set to true to disable ICAP preview.

  **default**: false

  .. versionadded:: 1.11.6

* preview_data_read_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout value for the read of preview data.
  If timeout, preview will not be used in the request send to the ICAP server.

  **default**: 4s

* respond_shared_names

  **optional**, **type**: :ref:`http header name <conf_value_http_header_name>` or seq of this

  Set the headers returned by ICAP server in REQMOD response that we should send in the following RESPMOD request.

  This config option now only apply to REQMOD service.

  **default**: not set

* bypass

  **optional**, **type**: bool

  Set if we should bypass if we can't connect to the ICAP server.

  **default**: false

.. _conf_value_audit_stream_detour_service_config:

stream detour service config
============================

**type**: map | str | int

Config the :ref:`Stream Detour <protocol_helper_stream_detour>` service.

For *str* value, the value will be treated as *peer* as described following.

For *map* value, the keys are:

* peer

  **optional**, **type**: :ref:`upstream str <conf_value_upstream_str>`

  Set the peer address.

  **default**: 127.0.0.1:2888

* tls_client

  **optional**, **type**: :ref:`rustls client config <conf_value_rustls_client_config>`

  Enable tls and set the config.

  **default**: not set

* tls_name

  **optional**, **type**: :ref:`tls name <conf_value_tls_name>`

  Set the tls server name to verify peer certificate.

  **default**: not set

* connection_pool

  **optional**, **type**: :ref:`connection pool <conf_value_connection_pool_config>`

  Set the connection pool config.

  **default**: set with default value

* connection_reuse_limit

  **optional**, **type**: :ref:`nonzero usize <conf_value_nonzero_usize>`

  Set how many times a single QUIC connection will be reused.
  The max allowed streams on this QUIC connection should be double of this value.

  **default**: 16

* quic_transport

  **optional**, **type**: :ref:`quinn transport <conf_value_quinn_transport>`

  Set the transport config for quinn.

  **default**: set with default value

  .. versionadded:: 1.9.9

* stream_open_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout to open QUIC streams to the detour server.

  **default**: 30s

* request_timeout

  **optional**, **type**: :ref:`humanize duration <conf_value_humanize_duration>`

  Set the timeout to get detour action response from the detour server after open the streams.

  **default**: 60s

* socket_buffer

  **optional**, **type**: :ref:`socket buffer config <conf_value_socket_buffer_config>`

  Set the socket buffer config for the socket to peer.

  **default**: not set

.. versionadded:: 1.9.8
