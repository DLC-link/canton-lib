pub mod google {
    pub mod rpc {
        tonic::include_proto!("google.rpc");

        pub mod context {
            tonic::include_proto!("google.rpc.context");
        }
    }
}

pub mod scalapb {
    tonic::include_proto!("scalapb");
}

pub mod com {
    pub mod daml {
        pub mod ledger {
            pub mod api {
                pub mod v2 {
                    tonic::include_proto!("com.daml.ledger.api.v2");

                    pub mod admin {
                        tonic::include_proto!("com.daml.ledger.api.v2.admin");
                    }

                    pub mod interactive {
                        tonic::include_proto!("com.daml.ledger.api.v2.interactive");

                        pub mod transaction {
                            pub mod v1 {
                                tonic::include_proto!(
                                    "com.daml.ledger.api.v2.interactive.transaction.v1"
                                );
                            }
                        }
                    }

                    pub mod testing {
                        tonic::include_proto!("com.daml.ledger.api.v2.testing");
                    }
                }
            }
        }
    }

    pub mod digitalasset {
        pub mod canton {
            pub mod v30 {
                tonic::include_proto!("com.digitalasset.canton.v30");
            }

            pub mod admin {
                pub mod crypto {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.admin.crypto.v30");
                    }
                }

                pub mod health {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.admin.health.v30");
                    }
                }

                pub mod mediator {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.admin.mediator.v30");
                    }
                }

                pub mod participant {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.admin.participant.v30");
                    }
                }

                pub mod pruning {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.admin.pruning.v30");
                    }
                }

                pub mod sequencer {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.admin.sequencer.v30");
                    }
                }

                pub mod time {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.admin.time.v30");
                    }
                }
            }

            pub mod connection {
                pub mod v30 {
                    tonic::include_proto!("com.digitalasset.canton.connection.v30");
                }
            }

            pub mod crypto {
                pub mod v30 {
                    tonic::include_proto!("com.digitalasset.canton.crypto.v30");
                }

                pub mod admin {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.crypto.admin.v30");
                    }
                }
            }

            pub mod mediator {
                pub mod admin {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.mediator.admin.v30");
                    }
                }
            }

            pub mod participant {
                pub mod protocol {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.participant.protocol.v30");
                    }
                }
            }

            pub mod protocol {
                pub mod v30 {
                    tonic::include_proto!("com.digitalasset.canton.protocol.v30");
                }

                pub mod v31 {
                    tonic::include_proto!("com.digitalasset.canton.protocol.v31");
                }
            }

            pub mod sequencer {
                pub mod admin {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.sequencer.admin.v30");
                    }
                }

                pub mod api {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.sequencer.api.v30");
                    }
                }
            }

            pub mod synchronizer {
                pub mod v30 {
                    tonic::include_proto!("com.digitalasset.canton.synchronizer.v30");
                }

                pub mod protocol {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.synchronizer.protocol.v30");
                    }
                }

                pub mod sequencing {
                    pub mod sequencer {
                        pub mod bftordering {
                            pub mod v30 {
                                tonic::include_proto!(
                                    "com.digitalasset.canton.synchronizer.sequencing.sequencer.bftordering.v30"
                                );
                            }

                            pub mod standalone {
                                pub mod v1 {
                                    tonic::include_proto!(
                                        "com.digitalasset.canton.synchronizer.sequencing.sequencer.bftordering.standalone.v1"
                                    );
                                }
                            }
                        }
                    }
                }
            }

            pub mod time {
                pub mod admin {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.time.admin.v30");
                    }
                }
            }

            pub mod topology {
                pub mod admin {
                    pub mod v30 {
                        tonic::include_proto!("com.digitalasset.canton.topology.admin.v30");
                    }
                }
            }

            pub mod version {
                pub mod v1 {
                    tonic::include_proto!("com.digitalasset.canton.version.v1");
                }
            }
        }
    }
}
