# generate resource files
"${RUN_DIR}"/mkcert.sh

# start nginx
[ -d /tmp/nginx ] || mkdir /tmp/nginx
/usr/sbin/nginx -c "${PROJECT_DIR}"/scripts/coverage/g3proxy/nginx.conf

# start g3fcgen
"${PROJECT_DIR}"/target/debug/g3fcgen -c "${RUN_DIR}"/g3fcgen.yaml -G port2999 &
FCGEN_PID=$!

# run g3proxy integration tests

export SSL_CERT_FILE="${RUN_DIR}/rootCA.pem"

g3proxy_ctl()
{
	"${PROJECT_DIR}"/target/debug/g3proxy-ctl -G ${TEST_NAME} -p $PROXY_PID "$@"
}

g3proxy_ftp()
{
	"${PROJECT_DIR}"/target/debug/g3proxy-ftp "$@"
}

set -x

for dir in $(find "${RUN_DIR}/" -type d | sort)
do
	[ -f "${dir}/g3proxy.yaml" ] || continue

	echo "=== ${dir}"

	"${PROJECT_DIR}"/target/debug/g3proxy -c "${dir}/g3proxy.yaml" -G ${TEST_NAME} &
	PROXY_PID=$!

	sleep 2

	[ -f "${dir}/testcases.sh" ] || continue
	TESTCASE_DIR=${dir}
	. "${dir}/testcases.sh"

	g3proxy_ctl offline
	wait $PROXY_PID
done

set +x

kill -INT $FCGEN_PID
NGINX_PID=$(cat /tmp/nginx.pid)
kill -INT $NGINX_PID

## g3proxy-ftp

echo "==== g3proxy-ftp"
g3proxy_ftp -u ftpuser -p ftppass 127.0.0.1 list
g3proxy_ftp -u ftpuser -p ftppass 127.0.0.1 put --file "${RUN_DIR}/README.md" README
g3proxy_ftp -u ftpuser -p ftppass 127.0.0.1 get README
g3proxy_ftp -u ftpuser -p ftppass 127.0.0.1 del README