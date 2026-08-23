(function () {
	"use strict";

	function decodeBase64(base64) {
		const text = atob(base64);
		const bytes = new Uint8Array(text.length);

		for (let i = 0; i < text.length; i++) {
			bytes[i] = text.charCodeAt(i);
		}

		return new TextDecoder().decode(bytes);
	}

	const container = document.getElementById("bgpmap");
	const viz = new Viz();

	viz.renderSVGElement(decodeBase64(container.dataset.graph))
		.then(function (element) {
			container.appendChild(element);
		})
		.catch(function (error) {
			const output = document.createElement("pre");
			output.textContent = String(error);
			container.appendChild(output);
		});
})();
