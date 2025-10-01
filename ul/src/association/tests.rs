
use dicom_core::{dicom_value, DataElement, VR};
use dicom_dictionary_std::{tags, uids::VERIFICATION};
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::entries::IMPLICIT_VR_LITTLE_ENDIAN;
// Helper funtion to create a C-ECHO command
fn create_c_echo_command(message_id: u16) -> Vec<u8> {
    let obj = InMemDicomObject::command_from_element_iter([
        // Affected SOP Class UID - Verification SOP Class
        DataElement::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, VERIFICATION),
        // Command Field - C-ECHO-RQ
        DataElement::new(tags::COMMAND_FIELD, VR::US, dicom_value!(U16, [0x0030])),
        // Message ID
        DataElement::new(tags::MESSAGE_ID, VR::US, dicom_value!(U16, [message_id])),
        // Command Data Set Type - No data set present
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            dicom_value!(U16, [0x0101]),
        ),
    ]);

    let mut data = Vec::new();
    let ts = IMPLICIT_VR_LITTLE_ENDIAN.erased();
    obj.write_dataset_with_ts(&mut data, &ts)
        .expect("Failed to serialize C-ECHO command");
    
    data
}
mod successive_pdus_during_client_association {
    use std::net::TcpListener;
    use super::*;
    use crate::{pdu::{PDataValue, PDataValueType}, ClientAssociationOptions, Pdu};

    use crate::association::server::*;

    #[test]
    fn test_baseline_sync() {
        // Immediately _after_ association, the server sends a C-ECHO command
        // This will be received by the client

        // Setup a mock server that will send multiple PDUs consecutively
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a second PDU (C-ECHO command) to send immediately after
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let server_pdu = echo_pdu.clone();

        // Spawn server thread that sends multiple PDUs back-to-back
        let server_handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.establish(stream).unwrap();

            // Send the second PDU (C-ECHO command) immediately after establishment
            association.send(&server_pdu).unwrap();
        });

        // Give server time to start
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create client and attempt association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // This should succeed in establishing the association despite multiple PDUs
        let mut association = scu_options.establish(server_addr).unwrap();
        
        // Client should be able to receive the release request that was sent consecutively
        let received_pdu = association.receive().unwrap();
        assert_eq!(received_pdu, echo_pdu);
        
        // Clean shutdown
        drop(association);
        server_handle.join().unwrap();
    }

    // Tests edge case where the server sends an extra PDU during association
    // client should be able to handle this gracefully.
    #[test]
    fn test_association_sends_extra_pdu_fails() {
        // During association, the server sends a C-ECHO command
        // This will be received by the client

        // Setup a mock server that will send multiple PDUs consecutively
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a second PDU (C-ECHO command) to send immediately after
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let server_pdu = echo_pdu.clone();

        // Spawn server thread that sends multiple PDUs back-to-back
        let server_handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            server_options.establish_with_extra_pdus(stream, vec![server_pdu]).unwrap();
        });

        // Give server time to start
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create client and attempt association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // This should succeed in establishing the association despite multiple PDUs
        let mut association = scu_options.establish(server_addr).unwrap();
        
        // Client should be able to receive the release request that was sent consecutively
        let received_pdu = association.receive().unwrap();
        assert_eq!(received_pdu, echo_pdu);

        // Clean shutdown
        drop(association);
        server_handle.join().unwrap();
    }

    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_baseline_async() {
        // Immediately _after_ association, the server sends a C-ECHO command
        // This will be received by the client

        // Setup a mock server that will send multiple PDUs consecutively
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a second PDU (C-ECHO command) to send immediately after
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let server_pdu = echo_pdu.clone();

        // Spawn server task that sends multiple PDUs back-to-back
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.establish_async(stream).await.unwrap();

            // Send the second PDU (C-ECHO command) immediately after establishment
            association.send(&server_pdu).await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create client and attempt association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // This should succeed in establishing the association despite multiple PDUs
        let mut association = scu_options.establish_async(server_addr).await.unwrap();
        
        // Client should be able to receive the release request that was sent consecutively
        let received_pdu = association.receive().await.unwrap();
        assert_eq!(received_pdu, echo_pdu);
        
        // Clean shutdown
        drop(association);
        server_handle.await.unwrap();
    }

    // Tests edge case where the server sends an extra PDU during association
    // client should be able to handle this gracefully.
    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_association_sends_extra_pdu_fails_async() {
        // During association, the server sends a C-ECHO command
        // This will be received by the client

        // Setup a mock server that will send multiple PDUs consecutively
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a second PDU (C-ECHO command) to send immediately after
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let server_pdu = echo_pdu.clone();

        // Spawn server task that sends multiple PDUs back-to-back
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            server_options.establish_with_extra_pdus_async(stream, vec![server_pdu]).await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create client and attempt association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // This should succeed in establishing the association despite multiple PDUs
        let mut association = scu_options.establish_async(server_addr).await.unwrap();
        
        // Client should be able to receive the release request that was sent consecutively
        let received_pdu = association.receive().await.unwrap();
        assert_eq!(received_pdu, echo_pdu);

        // Clean shutdown
        drop(association);
        server_handle.await.unwrap();
    }

    // Tests edge case where the client sends an extra PDU during association
    // using a broken client implementation that creates a new buffer instead of reusing it
    #[test]
    fn test_client_association_sends_extra_pdu_589_impl() {
        // During association, the client's broken implementation drops extra PDUs
        // This reproduces the behavior that #589 fixed

        // Setup a mock server
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) that should be lost
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};

        // Spawn server thread that sends extra PDUs during association
        let server_handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            
            // Use ServerAssociationOptions with extra PDUs during association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            server_options.establish_with_extra_pdus(stream, vec![echo_pdu]).unwrap();
        });
        // Give server time to start
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create client and attempt association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // This should succeed in establishing the association despite multiple PDUs
        let mut association = scu_options.broken_establish(server_addr.into()).unwrap();
        
        // Client should not have anything to receive
        let received_pdu = association.receive();
        assert!(received_pdu.is_err());
        
        // Client cannot receive the PDU that was sent during association
        // Clean shutdown
        drop(association);
        server_handle.join().unwrap();
    }

    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_client_association_sends_extra_pdu_589_impl_async() {
        // During association, the client's broken implementation drops extra PDUs
        // This reproduces the behavior that #589 fixed

        // Setup a mock server
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) that should be lost
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};

        // Spawn server task that sends extra PDUs during association
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            
            // Use ServerAssociationOptions with extra PDUs during association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            server_options.establish_with_extra_pdus_async(stream, vec![echo_pdu]).await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create client using broken implementation (creates new buffer)
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // Client's broken implementation will miss the extra PDU from server
        let mut association = scu_options.broken_establish_async(
            server_addr.into()
        ).await.unwrap();

        // Client should be able to receive the release request that was sent consecutively
        let received_pdu = association.receive().await;
        assert!(received_pdu.is_err());
        
        // Client cannot receive the PDU that was sent during association
        // Clean shutdown
        drop(association);
        server_handle.await.unwrap();
    }

}

mod successive_pdus_during_server_association {
    use super::*;
    use std::net::TcpListener;
    use crate::{pdu::{PDataValue, PDataValueType}, ClientAssociationOptions, Pdu, AeAddr};
    use crate::association::server::*;


    #[test]
    fn test_server_baseline_sync() {
        // Immediately _after_ association, the client sends a C-ECHO command
        // This will be received by the server

        // Setup server listener
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) to send immediately after association
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let client_pdu = echo_pdu.clone();

        // Spawn server thread
        let server_handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.establish(stream).unwrap();

            // Server should be able to receive the PDU sent by client after association
            let received_pdu = association.receive().unwrap();
            assert_eq!(received_pdu, echo_pdu);
        });

        // Give server time to start
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create client and attempt association, then send PDU immediately after
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // Establish association and send PDU immediately after
        let mut association = scu_options.establish(server_addr).unwrap();
        
        // Send the PDU immediately after establishment
        association.send(&client_pdu).unwrap();
        
        // Clean shutdown
        drop(association);
        server_handle.join().unwrap();
    }

    #[test]
    fn test_server_association_receives_extra_pdu() {
        // During association, the client sends an extra C-ECHO command
        // Server should be able to handle this gracefully

        // Setup server listener
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) to send during association
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let client_pdu = echo_pdu.clone();

        // Spawn server thread
        let server_handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.establish(stream).unwrap();

            // Server should be able to receive the extra PDU that was sent during association
            let received_pdu = association.receive().unwrap();
            assert_eq!(received_pdu, echo_pdu);
        });

        // Give server time to start
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create client that sends extra PDU during association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // Use the test method that sends extra PDUs during association
        let association = scu_options.establish_with_extra_pdus(
            AeAddr::new_socket_addr(server_addr), 
            vec![client_pdu]
        ).unwrap();
        
        // Clean shutdown
        drop(association);
        server_handle.join().unwrap();
    }

    #[test]
    fn test_server_association_receives_extra_pdu_589_impl() {
        // Reproduce behavior that #589 introduced
        // During association, the client sends an extra C-ECHO command
        // Server should _not_ be able to handle this gracefully

        // Setup server listener
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) to send during association
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let client_pdu = echo_pdu.clone();

        // Spawn server thread
        let server_handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.broken_establish(stream).unwrap();

            // Server misses the echo request entirely
            let received_pdu = association.receive().unwrap();
            assert_eq!(received_pdu, Pdu::ReleaseRQ);
        });

        // Give server time to start
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create client that sends extra PDU during association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // Use the test method that sends extra PDUs during association
        let association = scu_options.establish_with_extra_pdus(
            AeAddr::new_socket_addr(server_addr), 
            vec![client_pdu]
        ).unwrap();
        
        // Clean shutdown
        drop(association);
        server_handle.join().unwrap();
    }

    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_baseline_async() {
        // Immediately _after_ association, the client sends a C-ECHO command
        // This will be received by the server

        // Setup server listener
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) to send immediately after association
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let client_pdu = echo_pdu.clone();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.establish_async(stream).await.unwrap();

            // Server should be able to receive the PDU sent by client after association
            let received_pdu = association.receive().await.unwrap();
            assert_eq!(received_pdu, echo_pdu);
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create client and attempt association, then send PDU immediately after
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // Establish association and send PDU immediately after
        let mut association = scu_options.establish_async(server_addr).await.unwrap();
        
        // Send the PDU immediately after establishment
        association.send(&client_pdu).await.unwrap();
        
        // Clean shutdown
        drop(association);
        server_handle.await.unwrap();
    }

    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_association_receives_extra_pdu_async() {
        // During association, the client sends an extra C-ECHO command
        // Server should be able to handle this gracefully

        // Setup server listener
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) to send during association
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let client_pdu = echo_pdu.clone();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.establish_async(stream).await.unwrap();

            // Server should be able to receive the extra PDU that was sent during association
            let received_pdu = association.receive().await.unwrap();
            assert_eq!(received_pdu, echo_pdu);
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create client that sends extra PDU during association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // Use the test method that sends extra PDUs during association
        let association = scu_options.establish_with_extra_pdus_async(
            AeAddr::new_socket_addr(server_addr), 
            vec![client_pdu]
        ).await.unwrap();
        
        // Clean shutdown
        drop(association);
        server_handle.await.unwrap();
    }

    #[cfg(feature = "async")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_server_association_receives_extra_pdu_589_impl_async() {
        // Reproduce behavior that #589 introduced
        // During association, the client sends an extra C-ECHO command
        // Server should _not_ be able to handle this gracefully

        // Setup server listener
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Create a PDU (C-ECHO command) to send during association
        let echo_pdu = Pdu::PData { data: vec![
            PDataValue { 
                presentation_context_id: 1,
                data: create_c_echo_command(1),
                value_type: PDataValueType::Command,
                is_last: true 
            }
        ]};
        let client_pdu = echo_pdu.clone();

        // Spawn server task
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            
            // Use ServerAssociationOptions to establish the association
            let server_options = ServerAssociationOptions::new()
                .accept_any()
                .with_abstract_syntax(VERIFICATION)
                .ae_title("THIS-SCP");
                
            let mut association = server_options.broken_establish_async(stream).await.unwrap();

            // Server misses the echo request entirely
            let received_pdu = association.receive().await.unwrap();
            assert_eq!(received_pdu, Pdu::ReleaseRQ);
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create client that sends extra PDU during association
        let scu_options = ClientAssociationOptions::new()
            .with_abstract_syntax(VERIFICATION)
            .calling_ae_title("RANDOM")
            .called_ae_title("THIS-SCP")
            .read_timeout(std::time::Duration::from_secs(5));

        // Use the test method that sends extra PDUs during association
        let association = scu_options.establish_with_extra_pdus_async(
            AeAddr::new_socket_addr(server_addr), 
            vec![client_pdu]
        ).await.unwrap();
        
        // Clean shutdown
        drop(association);
        server_handle.await.unwrap();
    }
}
