# TLS autoassinado, com o certificado do Anfitrião fixado no Convite

O app trafega senhas e dados financeiros pela rede local, sem nuvem nem autoridade certificadora. O Anfitrião gera um certificado autoassinado e o app fixa a impressão digital dele no momento em que o Convite é aceito, recusando depois qualquer servidor com outro certificado. Escolhemos isso em vez de HTTP puro porque qualquer dispositivo no mesmo Wi-Fi poderia ler o tráfego, e porque adotar TLS mais tarde obrigaria todos os membros a refazer o pareamento.

## Consequences

- Trocar o certificado do Anfitrião (inclusive ao migrar de máquina) invalida o pareamento de todos os apps.
- Não há acesso pela internet: o Anfitrião só é alcançável na rede local, descoberto por mDNS.
