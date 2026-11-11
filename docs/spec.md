# Spec: Finanças da Residência (v1)

Vocabulário em [GLOSSARY.md](../GLOSSARY.md). Decisões de arquitetura em [ADR 0001](adr/0001-sincronizacao-por-versao-com-anfitriao-fixo.md) e [ADR 0002](adr/0002-tls-autoassinado-fixado-no-convite.md).

## Problem Statement

Um casal quer gerir junto as finanças da casa. Hoje não há um lugar único onde os dois registrem o que entra e o que sai, vejam a mesma informação a partir de cada computador e consigam antecipar como os próximos meses vão fechar. Eles não querem depender de um serviço na nuvem nem entregar dados financeiros a terceiros, e querem continuar lançando mesmo quando o outro computador está desligado.

## Solution

Um app desktop, instalado em cada computador da casa, em que os membros de uma Residência registram Receitas e Despesas e acompanham um dashboard com a situação do mês e a previsão dos seguintes. Um dos computadores é o Anfitrião da residência: os outros apps o encontram sozinhos na rede local e sincronizam com ele em tempo real. Cada app guarda uma cópia completa dos dados, então dá para lançar offline; quando o mesmo lançamento foi alterado em dois lugares, o app mostra as duas versões e a pessoa escolhe qual fica. O app se atualiza sozinho a partir do GitHub, sempre com a opção de adiar.

## User Stories

### Primeira abertura e acesso

1. Como pessoa abrindo o app pela primeira vez, quero escolher entre criar uma residência ou entrar com um convite, para começar sem precisar de uma conta em nuvem.
2. Como pessoa criando uma residência, quero definir meu usuário, minha senha e o nome da residência num único passo, para começar a usar em menos de um minuto.
3. Como criador de uma residência, quero me tornar automaticamente Administrador dela, para poder convidar os demais.
4. Como criador de uma residência, quero que meu computador passe a ser o Anfitrião dela, para que os outros apps da casa tenham com quem sincronizar.
5. Como usuário, quero entrar com usuário e senha nas aberturas seguintes, para que ninguém mais use meus dados.
6. Como usuário, quero permanecer conectado entre aberturas do app, para não digitar a senha todo dia.
7. Como usuário, quero sair da minha sessão, para que outra pessoa use o mesmo computador com o usuário dela.
8. Como usuário, quero ver a lista das minhas residências depois de entrar, para escolher em qual trabalhar.
9. Como usuário, quero adicionar uma nova residência a partir dessa lista, criando uma ou entrando com convite, para participar de mais de uma casa.
10. Como usuário que participa de residências em Anfitriões diferentes, quero vê-las todas no mesmo app, para não precisar de instalações separadas.
11. Como usuário, quero conseguir entrar e ver meus dados com o Anfitrião desligado, para não depender do outro computador.

### Convites e membros

12. Como Administrador, quero gerar um Convite com código curto, para trazer alguém para a residência.
13. Como Administrador, quero que o Convite expire em 24 horas e sirva uma única vez, para que um código esquecido não vire uma porta aberta.
14. Como Administrador, quero ver os convites pendentes e revogá-los, para corrigir um convite gerado por engano.
15. Como convidado, quero que o app encontre o Anfitrião na rede sozinho, para não precisar digitar endereço IP.
16. Como convidado, quero criar meu usuário e informar o código do convite na mesma tela, para entrar na residência de uma vez.
17. Como convidado, quero receber uma cópia completa dos dados da residência ao entrar, para ver tudo imediatamente.
18. Como convidado, quero uma mensagem clara quando o código é inválido, expirado ou já usado, para saber o que pedir ao Administrador.
19. Como Administrador, quero ver a lista de membros com seus papéis, para saber quem tem acesso.
20. Como Administrador, quero promover um membro Comum a Administrador, para dividir a gestão.
21. Como Administrador, quero rebaixar outro Administrador a Comum, para corrigir uma promoção.
22. Como Administrador, quero remover um membro, para encerrar o acesso de quem saiu da casa.
23. Como membro, quero que os lançamentos de um ex-membro continuem existindo e identificados como dele, para que o histórico não mude.
24. Como membro, quero que o sistema impeça a residência de ficar sem Administrador, para que ela nunca fique ingovernável.
25. Como Administrador, quero redefinir a senha de outro membro, para resolver senha esquecida sem e-mail.
26. Como único Administrador que esqueceu a senha, quero recuperá-la por um procedimento no computador anfitrião, para não perder a residência.
27. Como usuário, quero trocar minha própria senha, para mantê-la segura.
28. Como Administrador, quero excluir a residência inteira no Anfitrião, confirmando pelo nome dela, para encerrá-la sem risco de clique acidental.

### Lançamentos

29. Como membro, quero registrar uma Despesa com valor, data, descrição e categoria, para acompanhar o que sai.
30. Como membro, quero registrar uma Receita com os mesmos campos, para acompanhar o que entra.
31. Como membro Comum, quero que meus lançamentos fiquem vinculados a mim como Responsável, para que cada um responda pelo que lançou.
32. Como Administrador, quero escolher qualquer membro como Responsável ao criar ou editar um lançamento, para lançar em nome de outra pessoa.
33. Como membro, quero ver todos os lançamentos da residência, de qualquer Responsável, para ter a visão completa das finanças.
34. Como membro Comum, quero editar e excluir apenas os lançamentos em que sou Responsável, para não alterar por engano o que é de outra pessoa.
35. Como Administrador, quero editar e excluir qualquer lançamento, para corrigir erros de qualquer membro.
36. Como membro, quero filtrar lançamentos por mês, tipo, categoria, Responsável e situação, para encontrar o que procuro.
37. Como membro, quero buscar lançamentos pela descrição, para achar um gasto específico.
38. Como membro, quero que o valor seja sempre digitado como número positivo em reais, para não lidar com sinais.
39. Como membro, quero confirmação antes de excluir um lançamento, para evitar perda acidental.

### Categorias

40. Como criador de uma residência, quero que ela já venha com categorias padrão, para lançar sem configurar nada.
41. Como Administrador, quero criar, renomear e arquivar categorias, para adequá-las à nossa casa.
42. Como membro, quero que lançamentos antigos mantenham sua categoria mesmo depois de ela ser arquivada, para que o histórico não mude.
43. Como membro Comum, quero ver as categorias mas não alterá-las, para que a classificação fique consistente.

### Previsibilidade

44. Como membro, quero registrar um lançamento com data futura como Previsto, para planejar o que vem.
45. Como membro, quero marcar um lançamento Previsto como Efetivado, para registrar que ele aconteceu.
46. Como membro, quero ajustar o valor ao efetivar, para registrar o valor real de uma conta que veio diferente do previsto.
47. Como membro, quero que um Previsto cuja data passou apareça como atrasado em vez de se efetivar sozinho, para que nada conste como pago sem ter sido.
48. Como membro, quero criar uma Recorrência mensal, para não relançar salário, aluguel e assinaturas todo mês.
49. Como membro, quero definir uma data de fim opcional para a Recorrência, para representar contratos com prazo.
50. Como membro, quero que a Recorrência mantenha sempre doze meses de previstos à frente, para que a previsão do ano esteja sempre preenchida.
51. Como membro, quero escolher entre "só este" e "este e os próximos" ao editar um lançamento de uma Recorrência, para tratar exceções e reajustes.
52. Como membro, quero encerrar uma Recorrência removendo os previstos futuros e preservando os efetivados, para cancelar uma assinatura sem perder histórico.
53. Como membro, quero registrar uma compra como Parcelamento em N vezes, para que cada parcela apareça no mês certo.
54. Como membro, quero ver em cada parcela a sua posição no parcelamento (3 de 10), para saber quanto falta.
55. Como membro, quero excluir um Parcelamento removendo as parcelas ainda previstas, para corrigir uma compra cancelada.
56. Como membro Comum, quero que as regras de Responsável valham também para Recorrências e Parcelamentos, para que as permissões sejam as mesmas em todo lugar.

### Dashboard

57. Como membro, quero ver o dashboard ao entrar na residência, para saber a situação sem navegar.
58. Como membro, quero escolher o mês exibido, para olhar o passado e o futuro.
59. Como membro, quero ver receitas, despesas e saldo do mês, cada um separado em efetivado e previsto, para saber onde estou e onde vou fechar.
60. Como membro, quero ver as despesas do mês por Categoria, para saber para onde o dinheiro vai.
61. Como membro, quero ver o fluxo dos seis meses passados e dos seis futuros, com receitas, despesas e saldo acumulado, para enxergar a tendência.
62. Como membro, quero ver as despesas do mês por Responsável, para saber quanto cada um gastou.
63. Como membro, quero ver a lista dos próximos previstos, para saber o que vence em breve.
64. Como membro, quero ver a lista dos atrasados com um botão de efetivar, para colocar em dia o que já aconteceu.
65. Como membro, quero adicionar um lançamento direto do dashboard, para registrar um gasto com poucos cliques.
66. Como membro, quero que o dashboard distinga visualmente efetivado de previsto, para não confundir plano com fato.

### Anfitrião e sincronização

67. Como membro, quero que um lançamento feito em outro computador apareça no meu em poucos segundos, para que os dois vejam sempre o mesmo.
68. Como membro, quero criar, editar e excluir lançamentos com o Anfitrião inalcançável, para usar o app com o outro computador desligado ou fora de casa.
69. Como membro, quero ver um indicador de que estou offline e quantas alterações estão pendentes, para saber o que ainda não foi compartilhado.
70. Como membro, quero que as alterações pendentes sejam enviadas sozinhas quando o Anfitrião reaparecer, para não ter que lembrar de sincronizar.
71. Como membro offline, quero ser avisado de que categorias, membros, convites, Recorrências e Parcelamentos exigem conexão com o Anfitrião, para entender por que essas ações estão indisponíveis.
72. Como membro, quero que alterações em lançamentos diferentes sincronizem sem me perguntar nada, para só ser interrompido quando há uma escolha real.
73. Como membro, quero ver lado a lado a minha versão e a do Anfitrião quando há Conflito, para decidir qual fica.
74. Como membro, quero que editar de um lado e excluir do outro seja tratado como Conflito, para que nenhuma alteração se perca em silêncio.
75. Como membro, quero que os demais dados continuem sincronizando enquanto um Conflito espera minha decisão, para que um impasse não trave tudo.
76. Como membro, quero ver quantos Conflitos tenho pendentes, para não esquecê-los.
77. Como membro, quero que a comunicação com o Anfitrião seja cifrada, para que outros dispositivos no Wi-Fi não leiam nossas finanças.
78. Como membro, quero que meu app recuse um servidor que não seja o Anfitrião com que pareei, para que ninguém na rede se passe por ele.
79. Como membro com versão do app diferente da do Anfitrião, quero um aviso de que preciso atualizar para sincronizar, continuando a usar o app offline, para entender a situação sem perder acesso.
80. Como dono do computador anfitrião, quero que o app continue servindo a residência na bandeja do sistema depois de fechar a janela, para que os outros sincronizem sem eu manter o app aberto.
81. Como dono do computador anfitrião, quero a opção de iniciar o app junto com o sistema, para que o Anfitrião esteja no ar sempre que o computador estiver ligado.
82. Como dono do computador anfitrião, quero encerrar o app de verdade pelo menu da bandeja, para desligar o Anfitrião quando quiser.

### Backup e restauração

83. Como dono do computador anfitrião, quero backup automático diário da residência, para não perder tudo se o disco falhar.
84. Como dono do computador anfitrião, quero escolher a pasta dos backups, para apontá-la para outro disco ou pasta sincronizada.
85. Como dono do computador anfitrião, quero que só os últimos trinta backups sejam mantidos, para que não ocupem espaço indefinidamente.
86. Como Administrador, quero restaurar uma residência a partir de um arquivo de backup, para recuperar os dados depois de uma perda.
87. Como Administrador, quero restaurar uma residência a partir da cópia de outro computador, para reconstruir o Anfitrião ou migrá-lo de máquina.
88. Como membro, quero ser avisado de que preciso parear de novo depois que o Anfitrião foi restaurado em outra máquina, para entender por que a sincronização parou.

### Atualização

89. Como usuário, quero ser avisado ao abrir o app quando existe versão nova, para me manter atualizado sem visitar o GitHub.
90. Como usuário, quero confirmar a atualização e ter o app baixando, instalando e reiniciando sozinho, para atualizar com um clique.
91. Como usuário, quero recusar a atualização e continuar usando o app normalmente, para não ser interrompido no meio de uma tarefa.
92. Como usuário, quero que o app só instale atualizações assinadas pelo autor, para não receber um binário adulterado.
93. Como usuário sem internet, quero que a checagem de atualização falhe em silêncio, para que o app abra normalmente.

## Implementation Decisions

### Forma geral

- App desktop único em Tauri 2, para Windows e Linux. Não há executáveis separados de servidor e cliente: hospedar é uma capacidade do mesmo app, exercida na máquina onde a residência foi criada.
- Workspace Cargo com três crates:
  - **Domínio e sincronização**: regras de negócio, persistência e protocolo de sync. Não conhece Tauri nem HTTP. Expõe a interface de aplicação da residência.
  - **Servidor**: o Anfitrião, em axum, com endpoints de pareamento, sincronização e o canal em tempo real. Chama a interface de aplicação.
  - **App**: a casca Tauri (comandos, bandeja, inicialização com o sistema, updater, descoberta na rede) e o frontend.
- Frontend em React, TypeScript e Vite, com Tailwind, shadcn e Recharts. Fica deliberadamente fino: exibe o que as consultas devolvem e não recalcula totais, previsões nem permissões.
- Interface em pt-BR fixo, valores em reais, datas em dd/mm/aaaa, sem infraestrutura de tradução. Identificadores no código em inglês, com tradução consistente do glossário: Household, User, Member, Admin, Entry, Income, Expense, Assignee, Planned, Settled, Category, Recurrence, Installment plan, Host, Invite, Conflict.

### Interface de aplicação da residência

- Um conjunto de comandos e consultas, cada um executado em nome de um membro identificado. É o único caminho para ler ou alterar dados, usado igualmente pelos comandos Tauri e pelo servidor.
- Toda verificação de permissão acontece aqui, nunca no frontend nem nos handlers HTTP.
- O relógio é injetado, porque a situação de atrasado, o horizonte de doze meses da Recorrência e a validade do Convite dependem da data atual.
- As consultas do dashboard devolvem os números já agregados, um resultado por bloco.

### Dados

- SQLite via `rusqlite`, com migrações versionadas. Um arquivo por residência, o que permite a um mesmo app conhecer residências de Anfitriões diferentes.
- Valores monetários guardados como inteiros em centavos, sempre positivos; o tipo do lançamento dá o sinal. Moeda única.
- Identificadores gerados no cliente (UUID), para permitir criação offline sem colisão.
- Todo registro sincronizável carrega um número de versão e uma marca de exclusão desde a primeira fatia que persiste dados, mesmo antes de existir sincronização, para que ela não exija migração estrutural depois.
- Lançamentos gerados por Recorrência ou Parcelamento guardam referência à origem e, no caso das parcelas, a posição na série.
- Categorias são arquivadas, não removidas, de modo que lançamentos antigos mantêm a referência.
- Membros removidos permanecem como registro inativo, para que seus lançamentos continuem atribuídos.

### Identidade e sessão

- Usuários vivem nos dados do Anfitrião e são replicados para as cópias, o que permite entrar offline. Não há identidade global: o mesmo indivíduo em dois Anfitriões é dois usuários.
- Senhas guardadas com argon2.
- A sessão persistida fica no cofre de credenciais do sistema operacional.
- Recuperação de senha do único Administrador por um comando executado localmente no computador anfitrião.

### Sincronização (ADR 0001)

- Cada app mantém uma fila local de alterações pendentes de lançamentos. Ao alcançar o Anfitrião, envia cada alteração junto com a versão de que partiu.
- O Anfitrião aceita a alteração se a versão base é a atual, incrementando-a; caso contrário recusa e devolve o estado atual, e o app registra um Conflito.
- Conflito é por lançamento inteiro, sem merge por campo. Editar contra excluir também conflita.
- Resolver um Conflito é escolher entre a versão local e a do Anfitrião. Escolher a local a reenvia com a versão atual do Anfitrião como base; escolher a do Anfitrião descarta a alteração local.
- Um Conflito pendente não bloqueia a sincronização dos demais registros.
- Offline, apenas lançamentos avulsos podem ser criados, editados e excluídos. Categorias, membros, convites, papéis, Recorrências e Parcelamentos são operações executadas diretamente no Anfitrião e exigem conexão.
- O Anfitrião mantém um WebSocket com cada app conectado e notifica as mudanças; o app aplica e a tela se atualiza. Sem conexão, o app tenta reconectar em segundo plano.
- Cada versão do app declara um número de protocolo. Se o do app e o do Anfitrião diferem, a sincronização fica suspensa com aviso, e o app segue em modo offline.
- A geração de previstos de uma Recorrência acontece no Anfitrião, para que duas cópias nunca gerem a mesma série em duplicidade.

### Rede e segurança (ADR 0002)

- O Anfitrião só é alcançável na rede local e se anuncia por mDNS. Não há acesso pela internet.
- O Anfitrião gera um certificado TLS autoassinado na criação da residência. O app fixa a impressão digital dele ao aceitar o Convite e recusa qualquer outro certificado depois.
- O Convite é um código curto, de uso único, válido por 24 horas, revogável.
- Após o pareamento, cada requisição é autenticada por um token de sessão emitido pelo Anfitrião.
- A primeira execução como Anfitrião no Windows dispara o aviso do firewall; a tela de criação de residência deve antecipar isso ao usuário.

### Previsibilidade

- A situação Previsto ou Efetivado é um campo explícito do lançamento. "Atrasado" não é armazenado: é um Previsto cuja data é anterior a hoje.
- Efetivar aceita um valor diferente do previsto.
- A Recorrência é mensal na v1, com data de fim opcional, e mantém doze meses de previstos à frente. Editar com "este e os próximos" altera a regra e regenera os previstos futuros, sem tocar nos efetivados nem nos alterados individualmente.
- O Parcelamento gera N despesas previstas em meses consecutivos a partir do valor total, com a diferença de arredondamento alocada na primeira parcela.

### Operação

- No computador anfitrião, fechar a janela mantém o app na bandeja servindo a residência; encerrar de fato é uma ação do menu da bandeja. Iniciar com o sistema é opcional.
- Backup diário do arquivo da residência no Anfitrião, em pasta configurável, com retenção dos últimos trinta.
- Restaurar uma residência a partir de arquivo aceita tanto um backup quanto a cópia de outro computador, e transforma a máquina que restaura no Anfitrião. Como isso gera um novo certificado, os demais apps precisam parear de novo.

### Distribuição e atualização

- Repositório público no GitHub. GitHub Actions compila e publica os releases: instalador NSIS para Windows e AppImage para Linux.
- Auto-update pelo plugin de updater do Tauri, consumindo GitHub Releases, com a chave de assinatura nos secrets do Actions.
- A checagem ocorre na abertura do app. O usuário pode recusar e seguir usando; falha de rede na checagem é silenciosa.
- O instalador do Windows não tem certificado de code signing na v1.

### Ordem de construção

Cinco fatias, cada uma utilizável ao final:

1. **Esqueleto com auto-update**: app vazio, CI, release assinado, prompt de atualização funcionando no Windows.
2. **Uso local**: criar residência, login, lançamentos, categorias e dashboard, numa máquina só.
3. **Previsibilidade**: Previsto e Efetivado, Recorrência, Parcelamento e os blocos de previsão do dashboard.
4. **Anfitrião e sincronização**: TLS, mDNS, Convite, papéis, tempo real, edição offline e tela de Conflito.
5. **Operação**: bandeja, iniciar com o sistema, backup e restauração.

## Testing Decisions

- Um bom teste aqui descreve um comportamento observável por um membro ("um membro Comum não consegue editar o lançamento de outro", "A edita offline, B exclui, A reconecta e vê um Conflito") e o verifica pela interface pública, sem inspecionar tabelas, filas internas nem a ordem das chamadas. Deve continuar passando se o schema ou o formato do protocolo mudar sem alterar o comportamento.
- **Costura 1, a interface de aplicação da residência**: comandos e consultas executados contra um SQLite real em memória, com relógio injetado. Cobre permissões por papel e por Responsável, ciclo de vida de membros e a regra do último Administrador, categorias, Previsto e Efetivado, atraso, Recorrência (horizonte de doze meses, data de fim, "só este" contra "este e os próximos", encerramento), Parcelamento (posição, arredondamento, exclusão), validade e uso único do Convite, e todos os números de cada bloco do dashboard.
- **Costura 2, réplicas sincronizando**: um Anfitrião e dois ou mais apps montados no mesmo processo de teste, com o servidor real em loopback e TLS ativo. Cobre pareamento por Convite, recusa de certificado diferente do fixado, propagação em tempo real, edição offline com envio ao reconectar, Conflito de edição contra edição e de edição contra exclusão, as duas resoluções possíveis, sincronização dos demais registros com Conflito pendente, bloqueio de operações administrativas offline e suspensão por versão de protocolo diferente.
- Sem teste automatizado na v1: a casca Tauri (bandeja, iniciar com o sistema), o auto-update, a descoberta por mDNS e as telas React. Cada fatia termina com um roteiro de verificação manual cobrindo esses pontos, e o da fatia 1 inclui obrigatoriamente uma atualização real entre dois releases numa máquina Windows.
- Não há testes anteriores para servir de referência: o repositório começa vazio, e os primeiros testes de cada costura estabelecem o padrão.

## Out of Scope

- Contas bancárias, cartões, saldos e importação de extrato. O app controla fluxo, não onde o dinheiro está.
- Orçamento ou meta por categoria, alertas e notificações.
- Exportação de dados e relatórios.
- Tags, anexos e múltiplas moedas.
- Recorrências não mensais.
- Histórico de alterações e auditoria de quem editou o quê.
- Acesso pela internet, identidade em nuvem e recuperação de senha por e-mail.
- Aplicativo móvel e macOS.
- Troca automática de Anfitrião. Migrar de máquina é feito pela restauração a partir de arquivo.
- Merge de Conflito campo a campo.
- Outros idiomas.
- Certificado de code signing para o instalador do Windows.

## Further Notes

- O spec ainda não foi publicado em issue tracker: o repositório não existe. Quando existir, este arquivo pode ser publicado como issue com a label `ready-for-agent`.
- O instalador do Windows sem code signing faz o SmartScreen exibir um aviso no primeiro download. É esperado e ocorre uma única vez por máquina, já que as atualizações seguintes chegam pelo updater.
- A fatia 1 vem primeiro de propósito: é a de maior risco de infraestrutura (chaves, CI, formato dos releases) e é o canal que entrega todas as outras às duas máquinas da casa.
