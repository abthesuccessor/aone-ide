export async function loadShipment(shipmentId) {
  const response = await fetch(`http://127.0.0.1:4310/api/shipments/${shipmentId}`);
  if (!response.ok) {
    throw new Error(`Shipment request failed with ${response.status}`);
  }
  return response.json();
}

