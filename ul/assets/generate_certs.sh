# Generate test CA -- expires in 100 years
openssl req -x509 -nodes -days 36500 -newkey rsa:2048 \
    -keyout ca.key -out ca.crt -subj "/C=US/CN=MyTestCA"

# Generate server key and certificate signing request
openssl req -new -nodes -newkey rsa:2048 \
    -keyout server.key -out server.csr \
    -config server.cnf

# Sign CSR
openssl x509 -req -in server.csr \
    -CA ca.crt -CAkey ca.key -CAcreateserial \
    -out server.crt -days 365 -sha256 \
    -extfile server.cnf -extensions v3_req

# Generate client key and certificate signing request
openssl req -new -nodes -newkey rsa:2048 \
    -keyout client.key -out client.csr \
    -config client.cnf

# Sign CSR
openssl x509 -req -in client.csr \
    -CA ca.crt -CAkey ca.key -CAcreateserial \
    -out client.crt -days 365 -sha256 \
    -extfile client.cnf -extensions v3_req
